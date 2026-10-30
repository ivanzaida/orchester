# Storage: where the disk goes (ORC-424)

Status: draft for the person's review (2026-10-30). Nothing here is built yet; it follows ORC-422 (the disk guard
always works, keep free 50 GB by default, it lists what could be cleaned) and ORC-423 (ended runs drop their build
output, merged and canceled worktrees go).

## Where it is

File: the desktop design `crates/desktop-app/design/orchester.pen`, board **Storage · redesign** `GlCmZ`
(x 117460, y -561, right of *Roadmap · redesign*). Every window is an instance of `Window/Project` (`qlXhJ`), copied
from the Project settings sub-page *12 · Approved for every run* (`wQy6o`); the rows follow *Warm workspaces · design*
(`QaDAq`). Bars are instances of the Shell `Status bar` (`c6ZMl`) with the item slots overridden as on *Status bar ·
redesign*. Reused components: `Status/*`, `Button/Secondary`, `Button/Secondary/Disabled`, `Button/Ghost`,
`Button/Danger ghost`, `Button/Danger`, `Select/Default`, `Kbd`, `Tooltip`. No notice cards: states live in rows, the
status bar and the modal. Exports at scale 1: `.tmp/design/ORC-424/<node id>.png`.

| Row | Node | Frames |
| --- | --- | --- |
| Head | `AbCsw` | title and lead |
| Row 1 · Storage, the low-disk item and Clean up | `mSu5E` | `Jsbd1` 01 Project settings › Runs › Storage while the guard holds; `SqEDB` 02 status bar · Low disk space (runs paused, hover with tooltip, new steps wait, narrow 960); `X9BwMn` 03 Clean up confirmation modal |
| Row 2 · States | `LcKJr` | `yi3YU` 04 measuring sizes; `S5ek6a` 05 nothing to clean; `YwxsQ` 06 a worktree could not be removed |
| Row 3 · Loading and empty | `K7ZwWg` | `MfINi` 08 loading (the page opens, nothing read yet); `ubeHK` 09 empty (a new project, Orchester keeps nothing) |
| Row 4 · Entry and notes | `nHXUZ` | `meNMR` 07 the Runs tab's Storage row (calm, guard holds); `QU1YI` notes · what the design decides |

Every row fits the board's 6200 width (the board is 4533 tall).

## Decisions (proposed)

1. **Place.** Project settings › Runs › Storage is a sub-page of Runs (crumb *Runs / Storage*), reached from the
   Runs tab's *Storage* row and from the status bar item. The pane is 880 wide instead of the usual 720 so the size
   columns fit.
2. **Disk group.** *Disk H:* with free of total and a meter (Orchester's share, other files, the keep-free mark);
   *Keep free on H:* (50 GB); *Build output of waiting runs is cleared after* (6 hours); *Clean up* with what the
   ended runs keep. While the guard holds the disk hint is `warning` and names the paused runs.
3. **One list, largest first.** *What Orchester keeps* holds every run's worktree (task, run, state, path) and every
   warm workspace (number, its run, path) with *Size*, *Build output*, *Last used*. Row actions: *Clear build output*
   when the row has build output and no run works in it; *Remove* for ended runs and idle warm workspaces; *In use*
   while a run works or is paused there.
4. **Clean up** removes the worktrees of every ended run (Completed, Failed, Stopped at limit) after one modal: what
   goes with sizes, how much it frees and the free space after, what stays (branches, pull requests, artifacts) and
   what is not touched (waiting, paused and working runs, warm workspaces). The danger button names the size.
5. **Status bar.** *Low disk space · 32 GB free · 4 runs paused*: `warning` on the raised fill, left and pinned like
   interrupted runs, no dismiss; the whole item is the button and opens Storage, its tooltip says so. Below keep-free
   with nothing working it reads *new steps wait*. At 960 the words shorten; the free space and the count stay.
6. **States.** Loading: the page opens before anything is read; the disk row says *Reading the disk…* without a
   meter, the list is a `State/Loading` (*Reading what Orchester keeps…*), *Clean up* is disabled. Measuring: free
   space at once, sizes fill in (`…`), the group head shows *Measuring sizes* and the count, *Clean up* is disabled
   until counted. Empty: a new project with no worktree and no warm workspace; the meter has no Orchester share, the
   list is a `State/Empty` (*Orchester keeps nothing on this disk yet*, no action), *Clean up* disabled. Nothing to
   clean: the Clean up row says so and its button is disabled. Could not remove: the row turns `danger` and a
   full-width line under it names the process, its pid and the full path (*Held by cargo.exe (pid 18244) · H:\…*),
   with *Open folder* and *Try again*; the bar's latest event says what Clean up freed.
7. **Sample data agrees across a frame.** While the guard holds, all runs are paused, so the sidebar has no Running
   section; after Clean up (06) two runs work and two are queued, matching the sidebar and *Agents 2/4*. Group totals
   are the sum of the rows (160.1 GB, 130.2 GB). Times in the status bar are mono.
8. **Window height.** The windows are 1130 tall instead of the Read me's 900 so the whole list shows without a
   scroll; the app scrolls the pane at 900.

## Notes for the implementation

- Copy namespaces: `settings.project.runs.storage.*` (title, lead, disk, keep_free, build_output_after, clean_up,
  columns, row actions, states), `settings.project.runs.storage.clean_up.confirm.*` for the modal,
  `shell.status.low_disk.*` for the bar item and its tooltip.
- Nothing new in the design system: no new variables or components. The windows keep two transparent values
  (`#00000000`) copied from their source frame (sidebar row fill, tab underline stroke); the design system has no
  variable for transparent.
- The sidebar component instance carries an existing warning (`dU03e`, *Connections* with `fill_container` in a
  layout-less parent) from the shared `Sidebar v2`; it is not from this design.
