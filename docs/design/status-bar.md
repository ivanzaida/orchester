# Status bar redesign (ORC-222)

Status: draft for the person's review (2026-10-28). Nothing here is built yet; the implementation is a separate
task after approval.

The status bar today carries up to 15 items, 9 of them always on (`crates/desktop-app/src/shell/status_bar/`). The
redesign keeps only what is worth a glance: when nothing needs the person, the bar shows the latest event (or the
operation under way) on the left and one quiet health dot on the right. Everything else appears only while it
needs attention, or lives where ORC-108's redesign already shows it (Needs you in the title bar, the sidebar's
usage card, the project switcher).

## Where it is

File: the desktop design `crates/desktop-app/design/orchester.pen`, board **Status bar · redesign**
`bZVXG`, right of *Roadmap · wireframes*. Every bar is an instance of the Shell component `Status bar` (`c6ZMl`)
with its item slots overridden; actions are `Button/Size/Compact` (`SHKod`), dismiss is `Icon button/Compact`
(`ss3au`), the event tooltip is `Tooltip` (`rHJ7R`). Hover cards use the Menu's surface roles (`surface_raised`,
`border_strong`, `radius_tile`, `shadow`). Exports at scale 1: `.tmp/design/ORC-222/<node id>.png`.

| Row | Node | Frames |
| --- | --- | --- |
| Head | `CEp1o` | title and lead |
| Row 1 · Calm | `k4Fm2z` | `ShZkh` 01 Before (as built), `aexpB` 02 After · calm, `M0X3f` 03 After · one operation |
| Row 2 · In the window | `Eu8kD` | `g1Dt9O` 04 the window (`S7uiAk`) with the new bar and the usage card's plan lines |
| Row 3 · Attention states | `iVko9` | left: `vNIo3` 05 runner starting, `a7yNee` 06 runner offline, `xqwSg` 07 several operations, `fNb4q` 08 failure, `VIqZD` 09 interrupted runs, `CKlpO` 10 tasks unread; right: `X9S93` 11 auto-approving, `x4tTdZ` 12 account at its limit, `BfW9V` 13 team server offline / signed out, `jUXNw` 14 certificate changed, `UcdZT` 15 Cloud offline, `cKExU` 16 MCP not listening, `oEBfM` 17 window control and QA copy, `y0SDOa` 18 several at once |
| Row 4 · Tooltips and hover cards | `FMz93` | `fPWaD` 19 health dot, `nyZ38` 20 runner offline, `s1aln` 21 event tooltip, `n83Wh` 22 account at its limit, `RqCeq` 23 Cloud offline |
| Row 5 · Narrow and light | `X4kO71` | 960: `Dvou4` 24 calm, `vtjRN` 25 several at once, `HtiNI` 26 runner offline; light: `r3wRbJ` 27 calm, `wD779` 28 several at once, `p1O1qD` 29 runner offline |
| Row 6 · Gate and startup | `aLXS6` | `qx5qe` 30 gate before, `K8VUO` 31 gate after, `es58N` 32 startup (unchanged) |
| Row 7 · Notes | `Lij6B` | `OXLj2` before → after table, `b5f95` what the design decides, `m074X` open choices |

## Decisions (proposed)

1. **Calm bar: two things.** Left: the latest event (icon, words, time) or the operation under way; right end: a
   6 px `success` dot. No project name, counts, tokens, MCP address or version while all is well.
2. **The health dot holds the detail.** Its hover card: runner state and since when, agents at work of the
   slots, queued tasks, team server, MCP server address with agents connected, the version. A press opens the
   runner log (as the runner item does today).
3. **Attention items appear and go.** Left, one at a time as built: runner problem, failure nobody opened, tasks
   that could not be read, interrupted runs, operations, latest event. Right, in this order: runs auto-approving
   (*Stop*), an account at its limit, team server not connected, Orchester Cloud offline, MCP problem, agents
   control the window; then the dot, always last.
4. **Runner not online:** the dot leaves; the runner item (dot and words) takes the left with *Restart runner* and
   *Open runner log* as built.
5. **Plan limits move to the sidebar usage card** (the person's choice, 2026-10-28): one line per CLI under
   *Tokens today* (`Claude plan 42%`, `Codex plan 65%`). The bar shows an account only at its limit, in `danger`,
   with when it resets; its hover card shows the readings.
6. **Colours as built.** Danger: runner offline, account at its limit, certificate changed, MCP not listening.
   Warning: auto-approving, interrupted runs, server offline or signed out. Muted: Cloud offline, window control.
7. **Narrow window:** words give way first (time, run and server names), then labels; icons, counts and actions
   stay; the dot never gives way.
8. **Sign-in gate:** only the locked MCP item; the version moves into that item's hover card. **Startup:** the
   version only, unchanged.

## Before → after

| Item today | After | Where its information is |
| --- | --- | --- |
| Project name, folder in tooltip | Gone | Sidebar switcher (name and folder) |
| Runner online | Health dot | Dot's hover card; press opens runner log |
| Runner starting / offline with actions | Stays · attention | Left |
| Latest event | Stays | Left; full words in a tooltip |
| Operations under way | Stays | Left; *Background operations* popover |
| Failure nobody opened | Stays · attention | Left, raised fill |
| Tasks could not be read, *Try again* | Stays · attention | Left, raised fill |
| Runs interrupted when Orchester closed, *Review* | Stays · attention | Left, pinned |
| Agents 3/4 | Gone | Dot's hover card; sidebar *Running* |
| Queued 3 | Gone | Dot's hover card; Tasks › Queued |
| N need you | Gone | Title bar *Needs you* |
| Waiting requests with *Review* | Gone (open choice A) | Title bar *Needs you*; the task's Conversation |
| Tokens today | Gone (already built by ORC-108, merged 2026-10-28) | Sidebar usage card |
| Plan limits | Only at the limit | Sidebar usage card, one line per CLI |
| Runs auto-approving, *Stop* | Stays · attention | Right, first |
| Team server | Only when not connected | Dot's hover card; Settings › Orchester |
| Orchester Cloud offline | Stays as built | Right |
| MCP server address | Only on a problem; QA/dev builds keep *MCP :port · QA* | Dot's hover card; Settings › Automation |
| Agents control the window | Stays while on (open choice B) | Right |
| Version | Gone | Dot's hover card; startup bar; gate MCP hover card |

## Open choices for the person

- **A.** Remove the waiting-request line with *Review* (Needs you already counts and opens them), or keep it for
  one-click Review from any page?
- **B.** Keep *Agents control the window* as a muted item while it is on (proposed, a safety signal), or only in
  the dot's hover card?
- **C.** When the daily token budget is used up and new work waits, add an attention item, or leave it to the
  usage card's meter?

## Notes for the implementation

- The usage card's plan lines are drawn on a copy inside frame 04 only; the shared `Sidebar v2` component
  (`Zn8dR`) is unchanged and needs its own approval to change.
- The meter in that copy keeps the component's own 2 px radius (no variable for it).
- Copy: the bar's words reuse `shell.status.*`; new strings are the hover card's labels (*Agents*, *Queued*,
  *Team server*, *MCP server*, *Orchester*) and the plan lines (*Claude plan*, *Codex plan*), in the
  `shell.status.health.*` and `sidebar.usage.plan.*` namespaces.
