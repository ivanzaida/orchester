# First start and onboarding redesign (ORC-286)

Status: draft for the person's review (2026-10-28). Nothing here is built yet; the implementation is a separate
task after approval.

On a fresh start the Welcome screen today (`crates/desktop-app/src/screens/all_projects/welcome.rs`) leads only to
*Create your first project* and *Open project folder…*; joining a team server waits in Settings › Orchester, which
needs a project first. The redesign offers three equal ways in on the first start: add a project from a folder,
join a team, open a team's shared project. Joining, picking the shared project, linking it to a folder and its first
download happen in place on the start page.

## Where it is

File: the desktop design `crates/desktop-app/design/orchester.pen`, board **Startup · redesign** `qzzYw`,
right of *Agent testing · redesign*. Startup screens are instances of `Window/Signed out` (`rMiiU`) with its MCP item
off, so the status bar shows only the version (Status bar · redesign, decision 8); frame 14 swaps its title bar for
`Window controls/macOS` (`x6yXo`). All projects is `Window/Project` (`qlXhJ`) with the sidebar off and one *All
projects* tab. Controls are instances of `Button/*` (`iBDdP`, `O1AstI`, `YaUJX`, `WkRyb`, `pvyxy`, `CDeEc`),
`Field/*` (`v0ZiYM`, `fpYht`, `x5lObj`, `P6AYda`), `Callout/*` (`S0IsA`, `DOnNo`, `ulcbB`), `Badge/*`, `Toggle/Radio`,
`Tag chip`, `Avatar/Person`, `Brand mark` and `State/Empty` (`j5XsF`). Sample data: the Acme team on
`orchester.acme.dev`, its projects Kontur and Billing, the person maria@acme.dev. Exports at scale 1:
`.tmp/design/ORC-286/<node id>.png`.

| Row | Node | Frames |
| --- | --- | --- |
| Head | `zuFu8` | title and lead |
| Flow | `oPJyJ` | the steps with the frames that show them |
| Row 1 · First start | `D4yHz3` | `H9CZ1` 01 first start (dark), `ifctO` 01L the same in light, `Knacr` 01b signed in to Cloud already: the third way names the team and an invite |
| Row 2 · Join a team | `z4TXN` | `B914L` 02 join a team: Orchester Cloud or a code, `BD519` 03 signing in, `drCQd` 04 your teams and invites, `v5qbWM` 05 the team's shared projects |
| Row 3 · Open a shared project | `yzHyC` | `go2aM` 06 link to a folder (existing checkout, origin matches), `phoYc` 07 cloning with progress, `YN6MO` 08 first download with tasks arriving |
| Row 4 · Returning start | `d2ww3X` | `i5G19A` 09 All projects with a team's project offered above the list, `dLOCJ` 09b All projects, local only |
| Row 5 · States | `NPuAL` | `aWsVx` 10 wrong or expired code, `oOn6j` 11 server can't be reached, `NaEPI` 12 no shared projects yet, `sN8Pa` 13 folder is not a git repository, `tDFeg` 13b origin doesn't match the hint, `mruEH` 14 macOS: agent CLI not found (Finder-started app without the shell's PATH) |
| Row 6 · Notes | `i5TU1E` | `kcyR3` what the design decides, `OQiuc` open choices |

## Decisions (proposed)

1. **Three equal ways in.** *Add a project from a folder* (Choose folder…), *Join a team* (Sign in, Use a code),
   *Open a team's shared project* (Choose project…). None needs another first; each ends in an open project.
2. **No account to work locally.** Orchester Cloud is asked for only when the person joins a team (decided
   2026-10-28); the design assumes the sign-in gate is gone. The start page says so in one line.
3. **In place, no modals.** Join, pick, link and download are steps of the start page: a breadcrumb
   (*Start › Join a team › Acme › Kontur*), a step line (*Join the team, Choose a project, Link a folder, Download
   tasks*) and *Back*.
4. **Join.** Orchester Cloud sign-in lists the account's team servers and invites (`docs/architecture/server.md`
   rows 43–44); *Join* gives this computer its own device sign-in on that server. A code works without Cloud: the
   server address and an 8-character invite or pairing code (row 45).
5. **Projects.** A team lists only the projects the person is invited to (row 50). Before any team is joined, the
   third way goes through Join and then shows the projects.
6. **Folder.** An existing checkout: `origin` is compared with the project's remote hint; a mismatch warns and *Link
   anyway* decides (row 49). Or *Clone it here*, with progress in place (open choice A).
7. **First download.** Joined, linked, tasks (*84 of 212*, the latest arrivals listed), team settings, each with its
   state. *Open Kontur* works before the download ends; tasks keep arriving in the list without a restart (ORC-282).
8. **Returning start.** All projects stays as built (`crates/desktop-app/src/screens/all_projects/`): the centred
   1120 column, its description, the *Projects · Usage* tabs, the *Status*, *Active runs*, *Needs you* and *Last
   opened* columns, *Open project folder…*, *Add project* and the removal note under the list. It adds *Join a team*
   and *Open shared project…* to the header and a *Where* column (*Local*, or *Shared · Acme*). A team's project not
   on this computer yet is offered once above the list, with *Not now*. 09 is a Column `P5z2w` (tabs `k19me`,
   removal note `S3gIYI`); 09b is a Column `aPjYW` (tabs `z9cYpG`, removal note `olNkR`).
9. **macOS.** An app started from Finder lacks the shell's PATH; Orchester reads it from the login shell. When an
   agent CLI is still not found, the start page shows a warning callout with *Choose the claude file…*, *Check
   again* and the folders it looked in. Local projects still open.

## Open choices for the person

- **A.** Today Orchester never clones (the invite landing shows the `git clone` command). Build *Clone it here*
  (proposed), or keep the command with Copy?
- **B.** Before any team is joined, *Open a team's shared project* leads through Join (proposed), or is hidden until
  a team is joined?
- **C.** Settings › Orchester's join and invite landing pages could reuse these steps, so there is one join flow.
- **D.** There is no progress bar component; the bars are drawn from `surface_input` and `accent`. Add a Progress
  component to Components (needs approval)?

## Notes for the implementation

- Buttons and field boxes are 36 px (large) and 28 px as plain numbers: Pencil keeps no variable on a height. The
  values are `control_height_lg` and `control_height_md`.
- Gaps and paddings of 4, 8, 12, 16, 24 and 32 are `spacing_xs` … `spacing_section`; the others (2, 6, 10, 20, 28,
  40, 56) have no variable and stay numbers.
- An error under a field next to a button (13 `sN8Pa`: Folder row `d3Cx27`) sits under the row (`DNRz3`), so the
  button lines up with the field's box.
- Task states in the first download (08) are `Chip/*` instances; runner states in All projects are `Badge/Success`
  and `Badge/Neutral`. 01b `Knacr`, already signed in, offers *Your teams* instead of *Sign in*.
- The way-in cards, list rows, option panels and the progress bars are frames from the theme roles
  (`surface_raised`, `border`, `border_focus`, `radius_lg`, `spacing_*`); no shared component was changed.
- The macOS traffic lights keep the component's own colours.
- Copy: new strings go to `startup.*` (first start, the step line), `startup.join.*` (Cloud, code, teams, invites,
  errors), `startup.shared.*` (projects, link, clone, download) and `projects.header.*` (*Join a team*, *Open shared
  project…*, the team strip). The Cloud sign-in form reuses the Sign-in board's words.
- Decision 9 as built (`crates/desktop-app/src/screens/all_projects/agent_cli.rs`):
  - **Where.** The callout `SIbdo` and fix row `U4QvYO` of frame 14 `mruEH` sit on today's Welcome (`UQZRa`) and above
    All projects' list, 1120 wide rather than the frame's 1040. The first-start page frame 14 is drawn on (01 `H9CZ1`)
    is not built, so its Head, Ways, Local note and Footer are not either.
  - **A first start has no connection to fix.** The chosen file is kept in the application database (`ui_state`,
    `orchester-app.agentProgram.<program>`) and offered in Add connection.
  - **What counts as not found.** With listed projects, a CLI connection's program file is gone and the search path has
    no copy of that CLI. With none, neither CLI is on the search path and no chosen file exists; it then names `claude`.
    A connection whose program is gone while a copy is on the search path shows nothing, since "not found" would be
    false.
