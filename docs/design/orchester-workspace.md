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
| 02.01 · Orchestrator / New conversation | `dOIrp` | [Preview](previews/dOIrp.png) |
| 02.02 · Orchestrator / Task proposal | `I7TCI` | [Preview](previews/I7TCI.png) |
| 02.03 · Tasks / Select drafts to run | `J9467` | [Preview](previews/J9467.png) |
| 02.03 states · Task rows by run state (compact, see [Task library](#task-library-bulk-creation-and-scheduling)) | `r2al55` | not exported yet |
| 02.04 · Create multiple tasks / Review briefs | `OqmXc` | [Preview](previews/OqmXc.png) |
| 02.05 · Run selected tasks / Parallel | `W1Dq6` | [Preview](previews/W1Dq6.png) |
| 02.06 · Run selected tasks / Sequential | `IyJ5a` | [Preview](previews/IyJ5a.png) |
| 02.07 · Task queue / Running and waiting | `Z22wg` | [Preview](previews/Z22wg.png) |
| 02.08 · Project overview | `Twtqc` | [Preview](previews/Twtqc.png) |
| 02.09 · New task / Brief, context & limits | `l2YA4` | [Preview](previews/l2YA4.png) |
| 02.10 · Research running | `RD4pc` | [Preview](previews/RD4pc.png) |
| 02.11 · Review & approve plan | `x3lVC` | [Preview](previews/x3lVC.png) |
| 02.12 · Working task / Conversation | `jgqPe` | [Preview](previews/jgqPe.png) |
| 02.13 · Ready for review | `vpgrJ` | [Preview](previews/vpgrJ.png) |
| 02.A · Paused / Resume from checkpoint | `Oqmog` | [Preview](previews/Oqmog.png) |
| 02.B · Retry limit / Review recovery | `J3JZaT` | [Preview](previews/J3JZaT.png) |
| 02.B2 · Retry limit / Another attempt not available yet | `z2xET` | not exported yet |
| 02.C · Close with active work / Confirm | `Xz30C` | [Preview](previews/Xz30C.png) |
| 02.D · Close / Waiting for agents to save | `a0tTV` | [Preview](previews/a0tTV.png) |
| 02.E · Next start / Session ended abnormally | `o8DCd` | [Preview](previews/o8DCd.png) |
| 02.F · Runner stopped / Reconnecting | `MAn7V` | [Preview](previews/MAn7V.png) |
| 02.G · Runner reconnected | `YHS1v` | [Preview](previews/YHS1v.png) |
| 02.H · Runner could not restart | `EFY7I` | [Preview](previews/EFY7I.png) |
| 02.I · Activity / Live transcript | `SfXU1` | not exported yet |
| 02.J · Run permissions / Change for this run | `q4K1G` | not exported yet |
| 02.K · Blocked by permission / Needs you (later milestone) | `H03XN` | not exported yet |
| 02.L · Stopped at limit / Raise limit | `D6Nwxz` | not exported yet |
| 02.L2 · Stopped at step budget / Raise step budget | `A7nVbK` | not exported yet |
| 02.L3 · Stopped at daily project budget / Resume | `F2aBX` | not exported yet |
| 02.I states · Activity empty, loading, failed, not following (compact) | `hig8O` | not exported yet |
| Token budgets · input states and display rule (compact) | `T1n5BA` | not exported yet |
| 02.J states · Apply now pending → applied (compact) | `LJmTH` | not exported yet |
| 02.M · Project settings / Limits | `c7e729` | not exported yet |
| 02.M states · Default time limit select, open (compact) | `u5Vn2` | not exported yet |
| 02.N · Project settings / General | `QV1af` | not exported yet |
| Sidebar runner card states (compact) | `O5NoB1` | [Preview](previews/O5NoB1.png) |
| 03.01 · Pipeline / Feature delivery | `Q21HOA` | [Preview](previews/Q21HOA.png) |
| 03.02 · Publish v4 / Confirm | `zfjsK` | [Preview](previews/zfjsK.png) |
| 03.03 · Published v4 | `yf6XH` | [Preview](previews/yf6XH.png) |
| 03.04 · Pipelines library | `LRMQW` | [Preview](previews/LRMQW.png) |
| 03.05 · New pipeline / Name and starting point | `c3jD7` | [Preview](previews/c3jD7.png) |
| 03.06 · Empty pipeline / No steps yet | `angoF` | [Preview](previews/angoF.png) |
| 03.A · No pipelines yet | `MqRT7` | [Preview](previews/MqRT7.png) |
| 03.B · Draft has problems / Publish blocked | `UjHDL` | [Preview](previews/UjHDL.png) |
| 04.01 · Project connections | `a4Lqb` | [Preview](previews/a4Lqb.png) |
| 04.02 · Add agent provider | `h6CmJ` | [Preview](previews/h6CmJ.png) |
| 04.03 · Agent profiles / Assign connection & model | `WPmtS` | [Preview](previews/WPmtS.png) |
| 04.04 · Agent profile / Permissions | `T1rjv` | not exported yet |
| 04.05 · Connections / GitHub connection | `CHa9s` | not exported yet |
| 04.06 · Add GitHub connection | `D4mDB` | not exported yet |
| 04.A · Expired MCP access / Reconnect | `RZUKo` | [Preview](previews/RZUKo.png) |
| 04.B · Options the connection can't enforce are disabled (compact) | `W0Owo` | not exported yet |
| 04.C1–C5 · GitHub connection: not signed in, gh not found, wrong account, unrecognized output, missing scope (compact) | `Cr0P2`, `ve4G9`, `P89Dl`, `kqfaL`, `HwBI7` | not exported yet |
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
| 07.01 · Project secrets | `R99lt` | [Preview](previews/R99lt.png) |
| 07.02 · Add project credential | `tpF7R` | [Preview](previews/tpF7R.png) |
| 07.03 · Runtime approval / One deployment | `nuuZL` | [Preview](previews/nuuZL.png) |
| 07.A · Duplicate secret name | `m9Dhp` | [Preview](previews/m9Dhp.png) |
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

There are 70 full screen frames; earlier screen IDs are preserved. Screens remain 1440 × 960, at x 48 / 1568 / 3088 with 80px horizontal gaps, 88px row gaps and labels 32px above each frame. Compact recovery panels are counted separately. Section subgroups have a 96px gap.

- Getting started: first launch → project folder → project with detected GitHub repository → runner/agents → ready. Setup blocking, compact empty-project feedback (`RlwqW`), the *Change GitHub account or access* panel (01.D, x 1568 / y 426) and the ten GitHub detection states (01.C1–C10, x 2128–4808, rows at y 100, 512 and 980) are in `GojEY` (height 1400; section height 3920) below the main flow.
- Tasks & runs starts with project chat in `I0Ij1T`: new conversation → task proposal. `UIc5y` follows with task library, bulk creation, parallel/sequential scheduling and active queue, and the compact task-rows board `r2al55` (x 3088 / y 1148, row 2). The individual flow is in `D4pfd`: overview → new task → research → plan approval → working conversation → ready for review. Pause and retry are alternatives in `O4vgKt`, with 02.B2 `z2xET` at x 3088; runner/repository/loading recovery, status variants and interaction notes are in `y8TKW8`. Application lifecycle states (close with active work, runner connection, recovery on start) and the sidebar runner card states are in `Z7Fyr8`. Agent execution states (Activity transcript, run permissions, permission request, stopped at limit) with the compact Activity, Apply now and token budget states, plus Project settings / Limits and General (row 3, with the time-limit select board `u5Vn2` at x 1568 / y 2592) and the step-budget and daily-budget stops 02.L2 / 02.L3 (row 4, y 3260), are in `BnxJ1` (height 4312). Group positions are y 160 / 1376 / 3640 / 5904 / 7120 / 8216 / 10832 respectively; the section height is 15240.
- Pipelines: the visual editor is separate from task execution. `UJUi0` holds edit → publish confirmation → published. `t9zWUe` holds the library (the Pipelines navigation destination) → new pipeline → an empty pipeline in the editor. `rBy9L` holds the empty library and the publish-blocked editor. Group positions are y 160 / 1376 / 2592; the section height is 3808.
- Agents & connections: connections → add provider → assign connection/model in the agent profile (`kOGgX` row 1), then the profile's Permissions tab (04.04), the GitHub connection (04.05) and Add GitHub connection (04.06) in row 2; expired-access recovery, the compact state 04.B (options the connection can't enforce) and the GitHub connection states 04.C1–C5 (x 2108 / 2492 / 2876 / 3260 / 3644) are in `DLRW5` (now at y 2424; section height 3640).
- Instructions: rules → composed prompt → variable catalog; unavailable-variable state is below.
- Tools & imports: library → configuration → access policy in the first group. Select → resolve dependencies → ready to import is a separate complete row in `TSGMX`. Success/failure panels remain in `tjNmM`, now with the Create pull request settings (06.A, x 1040) and push-rejected recovery (06.B, x 1600); `tjNmM` is 640 high and the section height is 3328.
- Secrets & access: library → add credential → runtime approval; duplicate-name validation is below.
- Pull requests: `scsYh` contains the review inbox → line-specific fix request → verification on a new commit. `u77pi` contains the question/answer alternative, compact stale-revision and queued-request states, the stale-sync and rate-limited strips (08.E, 08.F at x 2064), the GitHub-unavailable screen (08.D at x 3088) and the wrong-account and missing-scope strips (08.G, 08.H at y 1200); `u77pi` is 1680 high. The section is at x 41728 / y 0, width 4896 and height 3152.

Section previews: [Getting started](previews/organization/K7IL6.png), [Tasks & runs](previews/organization/g7ncNr.png), [Pipelines](previews/organization/Y6XGt.png), [Agents & connections](previews/organization/wNy9i.png), [Instructions](previews/organization/MRxol.png), [Tools & imports](previews/organization/V8h6d.png), [Secrets & access](previews/organization/b3N1Ix.png).

## Components and tokens

50 reusable components are defined. Most are in Foundations; the composed secret inspector, effective instruction preview and inline notice have canonical definitions within their owning screens. The shared brand is canonical inside the sidebar. The conversation composer is canonical inside the task panel and reused by project chat and PR feedback. Screens use linked instances; content, icon, state and supported size are intended overrides.

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
| Window status bar | `pmOpt` |
| Window status bar operations popover | `E1msJS` |
| Focus ring overlay (design-only, see [Focus](#focus)) | `X9gPDr` |
| Field / Token budget (whole-number input with `tokens` unit) | `sN3du` |
| Control / Switch (off state; on = `$accent` fill, knob right) | `K8ItM` |
| Permissions / Command entry (allowlist chip with remove) | `bpmzN` |
| Activity / Transcript entry | `N4SlQq` |
| Activity / Step header | `mDnc3` |

Colors: canvas `#101010`, sidebar `#141414`, surface `#1A1A1A`, raised `#222222`, hover/selection `#2C2C2C`, border `#363636`, hairline `#2B2B2B`, text `#EEEEEE`, muted `#A3A3A3`, subtle `#8F8F8F`, primary action `#E6E6E6` with `#181818` text, focus ring `#EEEEEE` with a `#101010` gap (`focus-ring`, `focus-gap`). Variables also hold fonts, spacing and 7/12px control/panel radii. Status always includes text and an icon.

**Counts** (decided 2026-09-27): every count is the plain number — `2`, `12`, `0` — never zero-padded (`02`, `00`) and never a dash when it is known. This applies to the Overview metrics (`dZ2Zt`, e.g. `C95Z93`), the Overview *Needs you* heading count and the status bar. A dash (`—`) means only "not known yet" (agent slots while the runner is not online).

**Scroll gutter** (decided 2026-09-27, drawn in 04.01 `JuVaA` and 02.L2 `A7nVbK`): a side panel or column whose content can outgrow the window (connection inspectors, *This run*, the Summary column) scrolls its details in a viewport that reserves a 12px gutter at its right edge, so the bar never covers a value. The panel's right padding shrinks by the same 12px, so content still ends 24px (or 32px) from the edge. Thumb: 4px wide, `$border`, radius 2, centred in the gutter (4px from each side), 4px from the top and bottom of the viewport, length = track × visible / total and at least 24px; no track. It shows only while the content overflows. Actions pinned below the viewport (inspector buttons, the status detail) never scroll. Select menus keep their own overlay thumb (`YD8PV`).

## Window chrome

The application draws its own window chrome; the operating system's title bar and window border
are not shown. A 32px title bar in `$canvas` sits above every 1440 × 960 screen frame and a 28px
status bar (`pmOpt`, see [Status bar](#status-bar)) sits below it, so the window is
1440 × 1020 (32 + 960 + 28). The empty part of the title bar is the drag area (double-click
maximizes).

- Windows (`QQmsI`): minimize, maximize/restore and close on the right, 46 × 32 each, lucide
  icons at 14px in `$muted`; hover fills the control with `$hover`.
- macOS (`V70DN`): the three window buttons on the left, 12px, 8px apart, 12px from the edge,
  in the platform's standard colors — the only non-grayscale colors in the application.

Screens include neither bar; design verification captures the app window at 1440 × 1020 and
compares the title bar (top 32px) with the platform component, the screen area (next 960px) with
the screen frame, and the status bar (bottom 28px) with the matching `pmOpt` state. A worked
example is the window composition `cpoal` in Foundations.

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
| Agents | bot · *Agents* `2/4` | agent slots in use / configured (`settings`, runner) | Tasks › Queue (`Z22wg`) |
| Queued | clock · *Queued* `2` | queued task runs in the project | Tasks › Queue (`Z22wg`) |
| Needs you | message · *Needs you* `2` | runs waiting for the user: plan approvals, questions, reviews — exactly the count of the Overview metric *Need your input* and the Overview *Needs you* heading (decided 2026-09-27). The plain number; `0` in `$subtle`, never `00` or a dash | Overview › Needs you panel |
| Tokens | gauge · *Tokens today* `3.8M / 20M` | counted tokens (input + output, see [Token budgets](#token-budgets)) of all runs in the project today vs. the project daily token budget | **Not built:** usage accounting and a usage screen do not exist; until then the item is non-interactive with a tooltip |
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

`keJ9Z` is now 1123 high; its group `Tg1WX` is 1300 high, so the Focus group `xURvJ` moved to y 3656 and Foundations `cuxUu` is 7576 high.

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

**Narrow windows.** Drop the version first, then the token item, then labels (icon + count only). The
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

The task creation fixture includes four acceptance criteria, including cancelled OAuth, and shows the six steps from published Feature delivery v3. Create & run leads to research and planning; it does not bypass plan approval. The plan screen exposes plan v1, scope and acceptance criteria, with Approve plan & continue and Request changes. Requested changes return to Planner; a revised plan requires a new decision.

A run that reaches its time or token budget stops its agents, commits a checkpoint and waits as *Stopped at limit* until the user raises the limit ([Stopped at limit](#stopped-at-limit)). Pause first settles at a safe checkpoint and stops new scheduling/tool calls; only then may the UI claim Paused. Resume continues the same run and version snapshot. Cancel stops new scheduling and preserves work and artifacts; it does not undo completed external operations, and unresolved in-flight outcomes remain visible. Retry-limit recovery offers one explicitly authorized additional attempt within the remaining budget or revision of the task first. The exhausted example is an alternative Run 129 / Attempt 3; the normal Run 128 shows intermediate checkpoint `4b92ad0` and final reviewed revision `8f3a2c1`.

**02.B2 · Another attempt not available yet (`z2xET`, drawn 2026-09-27 from the app).** Until retry recovery ships, 02.B is shown with its actions visibly unavailable rather than hidden or faked. The run has failed, so the top bar has no run controls. Evidence: notice *Run checks still fails after 2 retries* · *Automatic execution has stopped. Review what the latest attempt left behind.*; *Latest attempt · 6a71bd4* with the progress rows of the steps that ran in it (*Complete · attempt 3 · 96k tokens · 7 min*, *Reported failed · attempt 3 · 18k tokens · 2 min*); *What Run checks reported* with the agent's own words in mono; *Open verification report* disabled (0.4); *Changes and messages are preserved* · *The work of every step stays committed on orc/github-auth. Nothing was merged into your branch.* Panel *Allow another attempt*: *Tokens used*, *Remaining budget*, then *Additional attempts* and *Resume from* as unavailable fields (`$sidebar`, value in `$subtle`, a 12px lock where the chevron was), the sentence *Allowing another attempt is not available in this version yet. The run stays failed and its work stays on its branch.*, and *Allow 1 retry & continue* / *Revise plan first* disabled (0.4). The *You can revise the plan…* line of 02.B is not shown. When retry recovery ships, 02.B (`J3JZaT`) applies unchanged.

## Pipelines library, creation and publishing

Pipelines in the navigation opens the library (`LRMQW`), not an editor. It lists the project's pipelines with name, step summary, step count, version state, the number of tasks that use it and the last publish date. Version state is always a status label with an icon: *Published v1*, *v3 · Draft v4 saved* (a published version plus unpublished changes) or *Draft v1 · not published* (never published, so no task can select it). Selecting a row opens the editor (`Q21HOA`). *Import from project* opens the existing import flow (`W4zGKx`) and creates independent local copies. A side panel explains the version chain: Draft saved → Publish vN → Runs keep their version. The fixture has four pipelines (Deploy staging, Feature delivery, Investigate issue, Release notes); Deploy staging, Feature delivery and Investigate issue match the pipelines named on the Tools screen.

*No pipelines yet* (`MqRT7`) explains in two sentences what a pipeline is and that a task can only run a published pipeline. It shows the starter Feature delivery with its six steps (Research → Plan (approval) → Implement → Run checks ∥ Review → Create pull request) using `XVkaB` rows. *Create from starter* creates draft v1 with those steps; *Start empty* creates draft v1 with no steps. Neither publishes nor starts a run. The starter refers to the project's own Explorer, Planner, Builder and Reviewer profiles and the Run test suite and Create pull request tools; missing ones are flagged in the editor and block publishing. The empty-library fixture hides the Tasks count, because tasks cannot exist without a pipeline.

*New pipeline* (`c3jD7`) is a modal over the library (`#000000A6` backdrop, `$surface` panel, `$radius-panel`, hairline header and footer). It asks for a name and shows the generated slug (`dependency-updates`), which is unique in the project. Start from is a choice between the starter and an empty pipeline, and the starter's step chain is shown in the option. *Create draft* creates draft v1 once, then opens the editor; a retry must not create a second pipeline. A name that produces an existing slug must be rejected with the existing pipeline named (not drawn). Choosing the starter leads to the editor with the six steps as draft v1; choosing Empty leads to `angoF`.

*Empty pipeline* (`angoF`) is the editor with no steps: a slot under ON TASK START invites *Add first step* (an agent profile or a tool), and a short explanation with *Add step* and *Use starter steps*. With no step selected, the inspector shows the pipeline itself: name, slug, optional description, trigger, *Published version: None yet*, *Used by: No tasks*, *Steps: 0*, and a notice that tasks can select the pipeline only after v1 is published. Run and Publish v1 are disabled; the toolbar says why ("Add a step before you can publish v1").

*Publish blocked* (`UjHDL`) is draft v4 with two problems: the Review step has no agent profile, and Plan waits for Review, which closes the cycle Plan → Implement → Review → Plan. A strip under the toolbar says publishing is blocked by 2 problems and that published v3 still runs; each problem is a button that selects the step or edge. The cycle edge is drawn and labelled on the canvas, the Review card says "No agent profile", and the inspector marks the profile field as required with a sentence under it. Publish v4 is disabled; Run v3 stays available because it runs the published version. Problems are computed from the saved draft; the exact validation list (missing profiles, tools or credentials, cycles, unreachable steps, a join that can never complete) is implementation work.

*Publish v4 / Confirm* (`zfjsK`) opens from Publish v4. It shows *Published now: v3* next to *After publishing: v4*, the changes from v3, and that runs already started or queued pinned v3 and keep it. *Publish v4* publishes once; a retry must not create v5. *Published v4* (`yf6XH`) shows the result: the version label becomes *Published v4*, the save status says there are no draft changes and that the next edit starts draft v5, Run becomes *Run v4*, Publish is replaced by *Version history*, and a notice repeats that in-progress and queued runs keep v3. Publishing never changes a run that already pinned a version; moving an active run to a newer version would be a separate explicit action and is not designed.

The library row is a new reusable component, `hn8qO` (Pipelines / Library row), canonical inside `LRMQW`. The screens otherwise reuse the sidebar, button, status label, select field, key/value, summary step, pipeline step and inline notice components. Proposed copy namespace: `pipelines.library.*`, `pipelines.create.*`, `pipelines.empty.*`, `pipelines.validation.*`, `pipelines.publish.*`. Change counts, run counts, dates and the Dependency updates / Release notes pipelines are illustrative fixtures. Library filtering, version history, archive/delete, rename and slug collision states are not drawn.

## Task library, bulk creation and scheduling

The user explicitly requested creating ten tasks and letting agents work on them either concurrently or in sequence. A pipeline defines the steps within one task; the task queue coordinates separate task runs. All of this remains within the selected project.

Tasks (`J9467`) is the main backlog, with All tasks, Drafts, Queue, Needs review and Done tabs. New task opens the existing single-task form; Create multiple opens `OqmXc`. Rows expose task ID, title, published pipeline, status and readiness/dependency, with selection and direct entry to task details/conversation. Run selected opens a scheduling review rather than immediately starting work.

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
| Failed (circle-x) | *Run failed* |
| Cancelled (square) | *Run cancelled* |
| Draft (pencil) | readiness (*Ready to queue*, *Needs a brief* …) |

The task table's *Status* column is 128 wide (was 112) so *Stopped at limit* fits, and *Dependency / readiness* is 184 (was 146) so its longest text fits on one line (`clVjO` cells `VizsW`, `S2kP9L`; heading `B5FaIC`). Interrupted, Cancelled and Draft were added to the status variants `P3HONM`. The selection checkbox is drawn unselected in `$muted` there, since selecting tasks for a run is not built.

Create multiple supports pasting one title per line, adding rows and editing each task's brief, acceptance criteria, pipeline, limits and dependencies. Pasting titles does not invent accepted requirements. The drawn state shows ten completed briefs, with Task 03 selected. Create 10 tasks creates ten drafts and returns to Tasks; it does not start agents. It remains valid to save incomplete drafts, but missing required details are surfaced before queueing. Creating and queueing must each be idempotent; a failed response must not result in duplicate tasks or runs on retry.

The fixture uses tasks ORC-129–138 in a separate sample backlog from the original overview. ORC-131, Add switcher keyboard shortcuts, depends on ORC-129, Add repository switcher, being merged to main. References to Task 01 before creation are mapped to the new local task identity during creation. All ten tasks use published Feature delivery v3, a 1.4M-token per-task budget and 30-minute run limit; the selected group cap is 14M tokens. These remain fictional values and records.

| Setting | Proposed behavior |
| --- | --- |
| Parallel (`W1Dq6`) | Start eligible tasks by priority up to the configured task-run limit. The fixture uses two concurrent tasks. A task waiting for plan approval still occupies a task-run slot. Dependency-blocked tasks have not started and occupy no slot; other eligible tasks may proceed. |
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

## Project orchestrator conversation

Orchestrator is a permanent project navigation entry between Overview and Tasks. It opens a project conversation without requiring a task. New conversation starts a separate discussion; History finds prior discussions within the selected project. The empty composition state is `dOIrp`; the ten-task proposal state is `I7TCI`. These are proposed product interactions, not an implemented chat service.

The project chat handles ideation, decomposition, queue coordination and cross-task progress questions. A task's Conversation stays scoped to that task/run, with Orchestrator or a participating agent as recipient. Its panel now explicitly says Task conversation and To: Orchestrator · this task. Opening a task or project chat preserves the other conversation and its draft. A project message is not silently broadcast into every agent's task context.

The header identifies the project Orchestrator profile and model; Agent profile links to the existing profile editor. Project instructions apply to the project coordinator, and Manage context shows attached files, selected tasks and applicable source versions. Chat text alone does not publish permanent instructions. Repository access is not proof the entire repository was read; responses must identify sources actually consulted. Tool credentials and raw secrets remain excluded. Provider/model names in this fixture have the same unverified illustrative status as the other model screens.

The shown user asks to plan from an attached scope.md. The Orchestrator proposes ten briefs with acceptance criteria, a merge dependency, parallelism of two tasks and a 14M-token aggregate cap. The proposal's local task numbers are not persisted task IDs. Review & create tasks opens the existing bulk brief editor (`OqmXc`) populated with that proposal. Selecting a proposed row opens its brief in that editor. Create 10 tasks then saves drafts and returns confirmed IDs/links to this conversation; it does not queue work. Run selected opens the existing parallel or sequential scheduling review with the proposal settings carried over. Edits preserve the conversation/proposal link. This is one connected flow, not a second task or scheduler implementation.

Natural-language queue commands use the same project actions, permission checks, scheduling limits and plan/tool approvals as direct UI controls. Resolve the intended task/group scope; ambiguous requests need clarification. An explicit authorized action need not acquire an unrelated extra chat approval. Changes and their outcomes remain inspectable with exact affected task links. Requests to pause scheduling hold new starts; pausing a working task is a separate action at its checkpoint. Progress answers read actual run state and link to tasks; an unavailable status source is reported as unavailable. Fresh authorization, missing credentials or a changed configuration cannot be bypassed through chat.

The composer is the existing `C3nu0z` promoted to Conversation / Composer and reused by linked references; `SGjO0` is the new Orchestrator / Proposed task row. Existing message, button, status, property and navigation components are reused. The composer is disabled while empty. Suggestions populate an editable prompt rather than dispatching an implicit operation. Enter adds a newline; Ctrl/Cmd+Enter sends. Escape closes History/context overlays and restores focus. History and context overlays are specified entry points, not additional drawn screens.

Proposed runtime states: preserve drafts on failed sends and show Retry; deduplicate message and action retries; distinguish Sending, Thinking, Delivered and action-confirmed outcomes. A response failure must not erase the user's sent message or existing proposal. Stop response only stops response generation, not running tasks. Provider access failure links to Connections while retaining the conversation. Loading must not impersonate a new empty conversation. Revalidate proposal/configuration versions before creation or launch; never apply stale settings silently. Conversation retention, context compaction and history search need an engineering contract. Proposed copy namespaces extend `orchestrator.*`, `conversation.history.*`, `conversation.context.*` and `orchestrator.proposal.*`.

At narrower widths the task proposal becomes an on-demand panel above the composer, keeping chat usable; collapse project navigation first. Keyboard focus, panel transitions, persistence, response streaming and actual routing require runtime acceptance tests. [Project chat overview](previews/organization/I0Ij1T.png).

## Task conversation

The task has a visible Conversation tab, an Open conversation entry on the review summary, and a conversation panel beside running/paused progress. The recipient defaults to Orchestrator; the picker can target a participating agent directly. Orchestrator routes messages, while direct messages retain their selected recipient. Messages belong to the task and record run, step, recipient and author.

Delivery states distinguish Queued, Delivered and Acknowledged. Acknowledgement does not prove the requested change was applied. Work-changing messages are delivered at the next checkpoint; Pause is the visible control for an urgent interruption. While paused, messages remain queued and are delivered before the next action on resume. Scope changes return to plan review rather than silently changing approved work.

Proposed interaction contract: failed or pending sends retain the draft; retries deduplicate the message; Enter adds a newline and Ctrl/Cmd+Enter sends. Escape closes the recipient picker and restores focus. Keyboard, delivery and persistence behavior remains to be implemented and tested.

## Token budgets

Decided 2026-09-25 (user): budgets are measured in tokens, not money. Every former money amount in the design is now a token amount: new task (`l2YA4`), bulk limits (`OqmXc`), group and per-task caps (`W1Dq6`, `IyJ5a`), orchestrator proposal (`I7TCI`), run snapshot rows (`RD4pc`, `x3lVC`, `jgqPe`, `vpgrJ`, `Oqmog`, `J3JZaT`), step budget (`Q21HOA`, `zfjsK`, `yf6XH`, `UjHDL`), setup run limits (`x1YpfN`, `Q7htyU`), PR fix request (`Rrmhf`), the Overview metric (`Twtqc` and the lifecycle backgrounds) and the status bar item (`pmOpt`, states `keJ9Z`). Fixture values: 1.4M per task, 14M per group, 400k per step, 20M per project per day. The fixture task "Add task cost breakdown" is now "Add task token breakdown".

**What counts.** Token counts come from the usage each CLI reports (per turn or per invocation), never from estimates. Counted = input + output (reasoning output is part of output). Cached input is shown separately (*Cached input · 1.2M · not counted*) and does not count toward any budget. Where a CLI reports cached tokens as part of input (OpenAI-style usage), counted input = input − cached; where it reports them separately (Claude's cache reads), they are simply excluded; cache writes count as input. Field names are unverified. Money fields (`total_cost_usd` and similar) are ignored. Because usage arrives after a turn, a run can end slightly above its budget; the UI shows the real figure, never a clamped one.

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
| empty | *Enter a token budget.* |
| `1.4M`, `1.5`, letters | *Use digits only, for example 1,400,000.* (example grouped for the locale) |
| `0` | *Enter a budget above 0 tokens.* |
| above the maximum | *A run budget can be at most {max} tokens (project setting).* — `{max}` is the configured maximum in compact form, e.g. 10M |

There is no minimum beyond more than 0 (user decision 2026-09-25).

**Where the maximum is set: Project settings › Limits (02.M `c7e729`).** The sidebar already has *Project settings*; it opens a page with *General* and *Limits* tabs (only Limits is drawn). Budgets are project resources, so the limit is a project setting, not an application setting. Limits holds: *Maximum run budget* (default **10,000,000 = 10M**; applies to task, run and raised limits), *Default run budget* (1,400,000; prefilled in New task and bulk creation; must not exceed the maximum), *Daily project budget* (20,000,000; status bar and Overview) and *Default time limit* (30 minutes of active agent time). Settings save on change (*All changes saved*). Changes apply to new runs: a run already above a lowered maximum keeps its budget but cannot be raised above the new maximum. The New task read-back names the maximum (*= 1.4M tokens · input + output · max 10M per run*).

*Default time limit* is a select (`bPPG9`, `Field / Select` 240 × 36), not a free input; the read-back under it is *active agent time*. Open states in `u5Vn2` (drawn 2026-09-27 from the app): the menu (`Select menu` rules) offers eight fixed choices — **15, 30, 45, 60, 90, 120, 180, 240 minutes** — which is exactly the menu's height without scrolling (266px); the selected one has the check, the default is 30 (`oLDmx`). A stored limit that is none of the choices (set by an earlier version or in the project file, e.g. 50) is added in ascending order and selected, so the select never shows a value it can't offer; nine options scroll with the overlay thumb (`gGmy3`). Choosing saves at once (*All changes saved*); choosing another value drops the extra one from the list. Singular: *1 minute*. The run's own limits (New task, raise panel) stay whole-minute inputs, because a run limit can be any value above what was used.

The run-limits picker in bulk creation (`OqmXc`, *Run limits / task*) is a select that shows the compact value and opens the same inputs.

**Where budgets live.** Task/run budget (new task, bulk, queue), group cap (queue), step budget (pipeline step), project daily budget (status bar, Overview). Exhausting the group cap stops further scheduling with a reason; exhausting a run or step budget stops the run at its limit (below).

## Stopped at limit

Decided 2026-09-25 (user): when a run reaches its time or token budget, its agents are stopped, a checkpoint is committed and the run pauses as **Stopped at limit**. It continues only after the user raises the limit.

**02.L (`D6Nwxz`)**, built on 02.A Paused. The page description says what happened (*Stopped at the token limit at 12:31*). The top notice names the limit and the checkpoint (*Stopped at the token limit · checkpoint 7c1e9f2*) and states that nothing runs until the limit is raised (*Run 128 used 1.4M of its 1.4M tokens. Builder was stopped after its turn and its work was committed as a checkpoint.*). Below it, *Raise the limit to continue*: each limit with what was used (*Used 1.4M of 1.4M · Implement stopped mid-step*, *Used 24 of 30 min of active agent time*) and its input with the read-back (*= 2M tokens · 600k more than now*; *6 min left · pauses don't count*). Both are whole-number inputs with a unit (`tokens`, `minutes`; the time field `dpfJo` was a select until 2026-09-27). The token input is focused because the token limit was hit; after a time stop the time field is. The single primary action states the result (*Continue with 2M tokens*; after a time stop *Continue with 45 minutes*). The explanation says what continuing does: a new invocation of the stopped agent from the checkpoint, repeating the unfinished part of the step; the new limit applies to this run only. *Cancel run* stays in the top bar. Working context shows *Budget 1.4M / 1.4M tokens · limit reached* and *Last checkpoint 7c1e9f2 · 12:31 · saved at limit*. The conversation panel shows *Stopped*; messages queue until the run continues. The status variant *Stopped at limit* (octagon-pause) is in `P3HONM` for lists and the queue; such a run is not counted as running.

**02.L2 · Step budget (`A7nVbK`)**, the Summary tab as the app draws it. Page description *Run 128 · Attempt 1 · Stopped at the step budget of Implement at 12:31*; top bar *Cancel run* only. Notice (octagon-pause) *Stopped at the step budget of Implement · checkpoint 7c1e9f2* · *Implement used 412k of its 400k-token step budget in Run 128. Builder was stopped after its turn and its work was committed as a checkpoint. Nothing runs until you raise the step budget.* (the real figure, above the budget, never clamped). The panel is **Raise the step budget to continue** with one row only — *Step budget* · *Used 412k of 400k · Implement stopped mid-step* — and one focused token input (`600,000`, suggested = budget + half, rounded up to 100k, at most the project maximum) with the read-back *= 600k tokens · 200k more than now*; no time row. Explanation: *Continue starts a new Builder invocation in the same session from checkpoint 7c1e9f2 and repeats the unfinished part of Implement. The new step budget applies to Run 128 only; the pipeline keeps its own.* Primary *Continue with 600k tokens*. Errors: *Enter more than the 412k tokens Implement already used.*, *A step budget can be at most 10M tokens, the maximum run budget (project setting).* Below the panel the run's progress rows (`l3hyb`), then *Latest activity* (*12:31 Implement · Builder stopped*); on the right *This run* (Pipeline, Branch, Base, Last checkpoint, Model, Budget, Cached input, Time limit, Active agent time, Permissions — the permissions value wraps right-aligned with *change ↗* under it).

**02.L3 · Daily project budget (`F2aBX`)**. The run's own limits were not reached, so there is **no raise panel**: the run continues with **Resume run** (primary, top bar, beside *Cancel run*), like a pause. Page description *… Stopped at the daily project budget at 16:48*; notice *Stopped at the daily project budget · checkpoint 7c1e9f2* · *This project's runs used today's token budget. Builder was stopped after its turn and its work was committed as a checkpoint. Resume the run tomorrow, or raise the daily budget in Project settings and resume it now.* Resume while the day's budget is still used fails with the existing capacity message (*Daily token budget used — new runs wait until tomorrow or raise it in Project settings.*) as a field error under the page heading (not drawn); the run stays stopped. The status bar shows *Tokens today 20M / 20M*.

**Step row status.** While a run is stopped at any limit, the stopped step's row in the progress list reads **Stopped at limit** (octagon-pause, `Stopped at limit · attempt 1 · 412k tokens · 14 min`), never *Paused* — *Paused* is only a pause the user asked for. The task list shows the same label (see [Task rows](#task-library-bulk-creation-and-scheduling)).

**Summary column scrolls.** With the raise panel the Summary column is taller than a 960px window (02.L2): it scrolls in its viewport with the [scroll gutter](#components-and-tokens) (`ZVdRa`, thumb `PigH4`); *Latest activity* sits at the bottom when there is room and below the fold when there is not.

**Wording, unified 2026-09-27.** Token counts arrive when a turn ends, so every token-based stop (run budget, step budget, daily budget) lets the turn that used it finish: the sentence is always *{agent} was stopped after its turn and its work was committed as a checkpoint.* (without a checkpoint: *… was stopped after its turn; there was nothing new to commit, or the commit failed and Activity says why.*). A time stop ends the agent at once: *{agent} was stopped and its work was committed as a checkpoint.* The read-back *{step} stopped mid-step* stays for all of them: the step itself is not done.

Runtime contract: on reaching a token limit the runner lets the turn that used it end (the agent gets a short grace to exit; starting another turn ends it at once), stops new tool calls, ends the agent CLI process tree, commits the worktree state as a checkpoint on the run branch, then records the state — only then may the UI claim *Stopped at limit*. A time limit ends the agent at once, then the same. If the commit fails, the state is *Stopped at limit · checkpoint failed* with the uncommitted changes listed (not drawn). A step-budget stop behaves the same and names the step; other steps running beside it finish their invocations as for a pause. A raised limit can never be below what was already used. The time limit counts **active agent time only** (user decision 2026-09-25): time paused, stopped, or waiting for the user (plan approval, questions, permission requests) does not count. This is stated wherever the limit is set or shown: New task (`l2YA4`, read-back under Time limit), run limits in `W1Dq6`/`IyJ5a`/`OqmXc`/`RD4pc`/setup (*30 min active agent time*), the raise-limit panel (*Used 24 of 30 min of active agent time*) and Project settings › Limits.

Proposed copy namespace: `runs.limit.*`.

## Activity tab

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
- **Invocation panel** (right, 320px): step and attempt, agent and model, start time, input / output / cached / counted tokens, run budget and the permissions row (opens 02.J). Footnote: counts come from the CLI's reported usage; cached input is not counted.
- **Follow:** while the run is live the list follows new entries (*Following live*; footer *Builder is working · last event 2 s ago*, a real timestamp, never fake progress). Scrolling up stops following; a centred *Jump to latest · 6 new* button appears above the bottom edge and resumes following.

Compact states (`hig8O`): *No activity yet* (queued run; says why), *Loading activity for Run 128* (spinner; no skeleton that pretends to be entries), *Couldn't load activity* (the run is unaffected; *Open runner log*, *Try again*) and scrolled-up with *Jump to latest*. Secret values never appear in the transcript: tool output passes through the same redaction as logs before it is stored or shown.

Reused: `DeizF` status labels, `ZGwlv` select, `tfGRt` rows, `u1XBP` buttons, `K8ItM` switch. `mBkeJ` (Activity / Event) and `J5MIT` (Conversation / Message) were considered: `mBkeJ` has 74px rows with a subtitle and no meta or output slot, too loose for a dense transcript; agent messages in Activity are run output, not a conversation. Hence the new entry and step-header components. Proposed copy namespace: `runs.activity.*`.

Implemented (M9, 2026-09-27): **Changes tab and Ready for review.** The Changes tab has no frame of its own; it shows the latest run's diff from its base revision to its latest checkpoint with the pull request screen's file list (`h8PZJa`, `j9NDM`) and diff (`rz27k`, `vEZ42`), under a toolbar in the style of `SrS47` that names the two revisions, the checkpoint's step and time, and *Checkpointed*. What the run's worktree holds beyond its checkpoint (untracked files included) follows under *NOT CHECKPOINTED* and reads *Not checkpointed*; it is read again while an agent works. The diff is read with git and cached per pair of revisions; changes larger than the pull request limits (300 files, 20,000 lines) are measured, not shown. Before the first checkpoint the tab says *No checkpoint yet*. An Activity file change is one entry per file (*M · open in Changes* ↗; the CLIs report no line counts) and opens that file in the Changes tab, which keeps the file in its path. 02.13 (`vpgrJ`) shows on the Summary tab when a run succeeded; *Review PR* (`o5Txa1`) and *Ask agent* (`QzsSV`) are unavailable with the reason under them (no step opens a pull request yet; the task conversation comes later), as is *Open conversation*. Not shown until their data exists: the topbar *Review PR #42*, evidence rows (the run's steps verify the checkpoint's revision instead), review artifacts and the instruction snapshots row. The base is the run's base revision, not a branch; *What changed* lists what the steps reported.

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

The mixed-provider fixture assigns Explorer and Reviewer to Claude · Kontur / Opus, and Orchestrator, Planner and Builder to Codex · Kontur / Sol. The Builder inspector contains separate Connection and Model selectors; provider-supported settings such as reasoning/context appear conditionally. Pipeline steps select a published project profile and inherit its model by default. A step can explicitly override the model, and the inspector shows the effective connection and model.

Connections (`a4Lqb`, section 04) lists GitHub · github.com, Codex · Kontur, Claude · Kontur and Project knowledge MCP; its tabs are All connections, Agent providers, GitHub (formerly Repositories) and MCP servers. The selected connection shows provider, execution location, authentication source, status, last check, available models and read-only Used by links. CLI connections (agent CLIs and `gh`) use the account the CLI is signed in to on this computer; Orchester never signs in, stores or sees tokens, and the project records which account it expects (`J48psR`). The GitHub connection is described in [GitHub](#github). Add connection (`h6CmJ`) configures an agent provider and validates access before saving. Expired MCP access (`RZUKo`) exposes Reconnect and preserves task work. Revocation blocks future calls immediately; ordinary profile publishing does not rotate or revoke credentials.

**Agent connection inspector (`JuVaA` in 04.01, redrawn 2026-09-27 from the app; replaces `ez6IT`).** 344 wide, `$sidebar`, padding 24 / 12 (right) / 24 / 24, gap 20. A details viewport (`xAfju`) scrolls with the [scroll gutter](#components-and-tokens) (thumb `nf9DG`); the actions are pinned below it (`K9gvh`, right padding 12 so buttons end 24 from the edge): *Test connection*, *Edit connection* and the status detail in Inter 10 `$muted` (*The CLI answers and is signed in. Orchester runs it with the account it is signed in to.*; for other states the matching `connections.detail.*` sentence). Details, in order: name, status label, then the rows **Provider** (*Anthropic*), **Program file** (Geist Mono 10, the path shortened in the middle to 36 characters: `C:\…\AppData\Roaming\npm\claude.cmd`), **Runs with** (*Node.js* for an npm launcher, *cmd.exe* for another command script), **Script** (Geist Mono 10, the launcher's JavaScript file `C:\…\claude-code\cli.js`; only for an npm launcher), **Version** (*2.0.14*; *Not checked yet* before the first check, *Unknown* when the check could not read it), **Execution** (*This computer*), **Authentication** (*CLI login*), **Last checked**; then *Available models* (heading, model chips, the source note in Inter 10 `$muted`, 12 apart), *Refresh models*, and *Used by* (heading and profile links, 12 apart). *Runs with* and *Script* appear only when what starts is not the program file itself; an `.exe` shows neither. In the fixture the details are taller than the window, so the viewport is cut at *Refresh models* and the thumb shows. The sentence about disconnecting was removed: the app has no remove action.

Sol, Luna and Opus are illustrative labels supplied during the design discussion. Provider adapters, public model identifiers, authentication options and supported settings were not verified by this design task. The implementation must obtain actual model catalogs and capabilities from each adapter and validate profile compatibility; the mockups do not establish working Codex or Claude integration.

## Agent permissions

Decided 2026-09-25 (user): permissions are set on the agent profile and can be changed during a run. They are part of the profile version, like connection and model: *Publish vN* makes them available to new runs, and a run keeps the permissions it launched with unless you change them for that run.

**Profile editor tabs.** The profile editor (`WPmtS`) now has three tabs under the page heading: *Profile* (identity, connection & model, role instructions), *Permissions* and *Used in pipelines* (count). The former "Used in pipelines" block moved to its tab. A one-line summary of the permissions stays visible at the right of the tab strip on every tab (*Edits files in the task worktree · 3 allowed commands · Network off*).

**Permissions tab (04.04 `T1rjv`).** Three settings, then an enforcement panel:

| Setting | Options (fixture: Builder) | Notes |
| --- | --- | --- |
| Repository access | *Read-only* · **Edit files in the task worktree** | Option cards (`lcwsy`). Edits are only ever inside the run's own worktree. |
| Commands | *Ask before running commands* (disabled, marked *later*) · **Allowed commands** · *Any command in the worktree* | The allowlist is edited as chips (`bpmzN`) plus *Add command*. Entries match by prefix: `pnpm test` also allows `pnpm test src/auth`. A command outside the list is denied and the agent is told why; it is not a failure of the run. *Ask* needs the permission-request flow (below) and stays disabled until it ships. |
| Network | Switch (`K8ItM`), **Off** | Off: package installs and web requests fail. |

The *Enforced by <CLI>* panel lists each setting with the mechanism the chosen connection uses and whether it is enforced. It comes from a capability check of the connection's CLI (*checked 14:02*), not from a static table. There is no bypass, "skip permissions" or "full auto" option anywhere, and no setting to add one; the panel says so. Wording in the UI is provider-neutral except in this panel, which names the CLI.

**Options a connection can't enforce are disabled (04.B `W0Owo`, compact; user decision 2026-09-25).** An option the chosen connection's CLI cannot enforce is not offered: it is disabled (45% opacity) with a lock icon and the reason under it (*Unavailable on this connection: Claude Code can't confine commands to the worktree.*). To use it, assign a connection that can enforce it. Fixture: Explorer on Claude · Kontur — *Any command in the worktree* is disabled; the network switch is locked **On** because Claude Code blocks its web tools but not network use by commands (*Off is unavailable on this connection … so network stays on. The run summary says so.*). Network-off follows the same rule because disabling it leaves a usable profile (network simply stays on, and says so). The exception: if the rule would leave a setting with no enforceable option (e.g. neither *Allowed commands* nor *Any command* is enforceable), the profile cannot run and publishing is blocked with that reason and a link to change the connection — Orchester never falls back to an unenforced option. When the connection changes, a selected option that becomes unavailable is reset to the most restrictive available one and the change is shown before publishing.

**Pipeline steps narrow, never widen.** The step inspector (`Q21HOA`, and the same inspector in `zfjsK`, `yf6XH`, `UjHDL`) shows *Repository access*, *Commands* and *Network* under the step limits. A step can only choose values at or below its profile's (read-only instead of edit, a subset of the allowlist, network off); wider values are not offered. A narrowed row says so (*Commands · narrowed* `git, pnpm test`) and a note names what the profile additionally allows (*Reviewer v2 also allows pnpm lint*). With no profile selected (`UjHDL`) a single *Permissions —* row and a note say that permissions come from the profile. Effective step permissions = profile ∩ step. The former single "Repository access" value is now the first of these rows.

**On the task page.** The run panels show the current agent's effective permissions as a row with a link: *This run* in 02.10 (`RD4pc`, *Explorer · Read-only · view ↗*) and *Working context* in 02.12 (`jgqPe`) and 02.A (`Oqmog`) (*Builder · Edit files · 3 commands · Network off · change ↗*). The Activity tab's invocation panel repeats it.

**Change for this run (02.J `q4K1G`).** The link opens a popover anchored above the row (same surface as the status bar popover: `$raised`, `$border`, 12px radius, shadow 0 8 24 #00000066; 460px wide). It shows the agent, step, model and the profile version the values came from, then the three settings in compact form, with changed values outlined (fixture: `pnpm build`, *added for this run*). Timing, stated in the popover: **changes apply from the next agent invocation; the invocation running now keeps the permissions it started with.** This is the honest rule for CLIs that read permissions at launch. An invocation is one launch of the agent CLI (a step execution, a retry, a continue after pause or limit). Two ways to apply, both for Run 128 only: *Apply from next invocation* (primary) and *Apply now* (user decision 2026-09-25). *Apply now* lets the agent finish the command it is running, stops it at the next checkpoint, commits the worktree as a checkpoint and restarts the current step from it as a new invocation with the new permissions. Pending state (`LJmTH`, left): the permissions row reads *Applying at next checkpoint…* with a loader notice that says what happens to in-flight work — the running command completes, nothing in the worktree is lost, the agent's unsaved reasoning since its last turn is not carried over unless the CLI can resume its session — and *Keep for next invocation instead* cancels the restart. Applied state (right): *New permissions active since 12:09*, the new checkpoint (*saved to apply permissions*) and invocation 2; the first invocation's transcript stays in Activity. *Also save to the Builder profile* adds the same change to the profile draft (v3); new runs get it only after publishing. Escape or *Cancel* closes without changes and returns focus to the row. 

**Blocked by permission (02.K `H03XN`) — later milestone, not v1.** When an agent asks for something outside its permissions (fixture: `pnpm add @octokit/auth-oauth-app`, which also needs network), the task page shows a *Needs you* request card at the top of the run: the exact command, the worktree, what else it needs (network), the agent's stated reason, and three decisions — *Deny* (the agent is told and continues without it), *Allow once* (this exact command, one time, with network for that command) and *Allow for this run* (adds `pnpm add` and network access to this run's permissions from now on). The agent waits on that command. **Timeout: 10 minutes, then denied** (user decision 2026-09-25); the card says *denied automatically in 9:42 (at 12:18) if you don't answer*, and on expiry the card is replaced by a *Denied · no answer* entry in Activity. The request counts in *Needs you* in the status bar and Overview, and takes the status bar activity slot with its countdown (state I, `OwnMs`). It needs a CLI hook that asks Orchester instead of denying: Claude Code's permission-prompt tool can route the decision to an MCP tool the runner serves; `codex exec` has no such hook (Codex's app-server protocol has approval requests). Both unverified. In v1 a command outside the permissions is denied and appears as a *denied* entry in Activity (`SfXU1`); the user can then change the run's permissions (02.J).

**How settings map to CLI flags (implementation notes — every item is unverified until checked against the installed CLI versions, on Windows and macOS).**

| Setting | Codex CLI (`codex exec`) | Claude Code (`claude -p`) |
| --- | --- | --- |
| Working directory | `--cd <worktree>` | process cwd = worktree |
| Read-only | `--sandbox read-only` | deny edit tools: `--disallowedTools Edit Write NotebookEdit` (or `--permission-mode plan`) |
| Edit files in the worktree | `--sandbox workspace-write` | `--permission-mode acceptEdits` |
| Allowed commands | no flag; Codex exec-policy prefix rules, if they apply to `exec` | `--allowedTools "Bash(git:*)" "Bash(pnpm test:*)"`; anything else is denied in `-p` mode |
| Any command | `workspace-write` sandbox (commands confined to the worktree) | `--allowedTools Bash`; **not confined to the worktree** unless Claude Code's own sandbox is enabled → option disabled |
| Network off | default of `workspace-write` (sandbox network access off) | `--disallowedTools WebFetch WebSearch`; commands keep network → *Off* disabled, network stays on |
| Network on | sandbox network access enabled through a `-c` config override | web tools allowed |
| Ask (later) | app-server approval requests | `--permission-prompt-tool <mcp tool>` |
| Never used | `--dangerously-bypass-approvals-and-sandbox`, `--full-auto` | `--dangerously-skip-permissions`, `--permission-mode bypassPermissions` |

The fixture shows all three Builder settings enforced by Codex. If exec-policy rules turn out not to apply to `codex exec`, *Allowed commands* becomes a disabled option for Codex profiles (then only *Any command in the worktree* remains, which the sandbox enforces). Sandbox behaviour differs by platform (Codex's sandbox on Windows is the least certain); the capability check must run per machine, and whatever is missing is disabled with its reason, never silently treated as enforced.

Proposed copy namespaces: `agents.permissions.*`, `agents.permissions.enforcement.*`, `pipelines.step.permissions.*`, `runs.permissions.*`, `runs.permissionRequest.*`.

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
| Run | `run.revision` | string | After the run's first checkpoint commit |
| Step | `step.id` | string | Step selected |
| Step | `step.name` | string | Step selected |

The sample syntax is `{{task.title}}`. Variables are evaluated as data, not executable expressions. Secret values are excluded from this catalog. Missing values must not silently become empty text: explain the unavailable phase and offer an appropriate injection point or an explicitly configured fallback.

`run.base_revision` is the revision the run started from. `run.revision` is the run's latest checkpoint commit; a step's prompt gets the latest checkpoint at the time its invocation starts. Before the first checkpoint it has no value: an agent whose prompt uses it does not start, and its step fails naming the variable.

The unavailable-variable frame demonstrates `run.revision` before the first commit. Required policy blocks injection; the user can choose a later injection event or explicitly configure a fallback. The UI must not fabricate a revision or silently insert an empty string.

## Project tools and imports

Tools, secrets, instructions, agent profiles, pipelines and connections are all project-owned. The navigation entry opens an explicit project context; the breadcrumb and project selector show the current owner. A pipeline selects tools and profiles available in its project. Connections represent provider/server authentication within the project; Tools represent concrete callable actions. Their exact backend relationship remains a separate engineering contract.

The tool library offers creation of HTTP, command and database tools and discovery/import from an MCP connection. Setup owns action configuration, input/output mapping, credentials, allowed environment, access rules and version. The sample HTTP deployment tool selects `STAGING_DEPLOY_TOKEN` from Kontur and maps it to bearer authentication. The example deployment endpoint is fictional. Configuration validation must not execute a deployment. A future test action must clearly expose its actual side effects and apply the normal access policy.

A tool may only bind secrets owned by its project. Tool settings offer New project secret, returning the newly created reference to the binding selector. Secret settings have no tool picker or editable tool policy: Used by is a derived read-only list linking to the consuming tool. The secret creation screen collects name, type, value and environment in its current project.

Import resources is available in project navigation and from resource libraries. It covers pipelines, agent profiles, instructions, tools and secrets. The proposed default is an independent copy with a new local identity and source provenance. Dependencies must be enumerated: pipeline steps reference local agent/tool/instruction versions, and tools reference local connections and secrets. Tool credential dependencies are mapped to an existing compatible target-project secret, imported as a new secret, or supplied with a new value. Standalone secret import is also supported. Connections are checked again in the target project before use (for CLI connections: program, sign-in and the target project's expected account); importing configuration must not silently confer access. No credential value is displayed in the preview or placed on the clipboard.

The import fixture uses Select resources → Resolve dependencies for Preview release v3 from Personal website into Kontur. Seven mapping rows show the pipeline and six dependencies: reuse local Builder published v2 and Implementation standards v2; copy Build preview; rename the copied Deploy preview to Website preview because Kontur already contains a draft Deploy preview; import PREVIEW_HOST_API_KEY; and authorize Preview host configuration in Kontur. The library shows that existing draft among five tools, with two existing secret bindings.

The result is four new resource copies, two reused local resources and one separately authorized connection. The blocked and ready states differ in destination authorization; source access is never copied. The target policy is explicit: Build preview → Build, Website preview → Deploy, Preview environment, each deployment requires approval, all other callers denied. New configuration stays draft until reviewed and published; import does not start a run. Source/version and destination mappings remain inspectable.

Source edits do not propagate. Permission to read/import source resources and create target resources must be checked independently. Name collisions require an explicit rename or reuse selection, never a silent overwrite. Missing credentials leave the tool unready. Import failure preserves mappings and the original project and allows an idempotent retry without duplicate resources. Success opens the imported pipeline draft for review.

## Secrets and access

The proposed design stores credentials separately from prompt rules and task context. A secret has an owner project, environment, name, type and value. The tool owns credential references, operation constraints and step access rules. Values are masked in management UI; replacing a value rotates it. Used by is a read-only reverse reference to local tools. No actual secrets exist in this design file or its previews.

Agents request capabilities through named tools. The tool receives the credential for the allowed operation/window; the model must not receive raw secret values through prompts, variables, copied instructions, logs or tool output. Implementation must keep credentials out of agent-readable process state and redact tool output; a mask in the UI is not a security boundary. Storage encryption, key management, runner isolation, provider adapters and rotation are open engineering decisions requiring separate review.

The sample database capability uses a database account with read-only privileges and a constrained query tool. The deployment tool binds a token and grants its `deploy.staging` capability to a specific step. Read-only guarantees require database/tool enforcement, not instructions asking the agent to avoid writes. Rotation and archival remain secret lifecycle actions; usage remains observable from the secret, while editing permissions happens in Tools.

The Access tab (`H97stw`) edits the allowed pipelines and steps, environment, approval policy, access window and denied behavior. Configured Used by links are distinct from the allowlist. The selected policy asks for each deployment; policy configuration may permit previously authorized scopes without repeated prompts. The shown deployment approval names project, environment, run, step, action, exact revision, tool and credential reference. It authorizes one deployment and expires when the step finishes. A different revision or operation requires a fresh policy decision. Denial leaves the step awaiting a changed plan; it does not imply a crash or erase work. All other callers are denied. Revoke access is a separate immediate action.

Add-secret's successful fixture uses the unique name STAGING_READ_API_KEY. The duplicate state shows STAGING_DEPLOY_TOKEN already exists in Kontur / Staging, disables Save and links to the existing secret. Replace value remains the distinct rotation action.

Audit records contain who/what requested access, capability, scope, run/step, decision, time and outcome without credential values. Revocation affects future calls immediately; completed external side effects are not reversed. Network failure must not display a successful save, deployment or query. Retries need idempotency and explicit outcome handling.

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
