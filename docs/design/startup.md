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
| Row 3 · Open a shared project | `yzHyC` | `go2aM` 06 link to a folder (existing checkout, origin matches), `phoYc` 07 cloning with progress (not built: ORC-584 clones in a console, see "Clone in a console"), `YN6MO` 08 first download with tasks arriving |
| Row 4 · Returning start | `d2ww3X` | `i5G19A` 09 All projects with a team's project offered above the list, `dLOCJ` 09b All projects, local only |
| Row 5 · States | `NPuAL` | `aWsVx` 10 wrong or expired code, `oOn6j` 11 server can't be reached, `NaEPI` 12 no shared projects yet, `sN8Pa` 13 folder is not a git repository, `tDFeg` 13b origin doesn't match the hint, `mruEH` 14 macOS: agent CLI not found (Finder-started app without the shell's PATH) |
| Row 6 · Notes | `i5TU1E` | `kcyR3` what the design decides, `OQiuc` open choices |

## Decisions (proposed)

1. **Three equal ways in.** *Add a project from a folder* (Choose folder…), *Join a team* (Sign in, Use a code),
   *Open a team's shared project* (Choose project…). None needs another first; each ends in an open project.
2. **No account to work locally.** Orchester Cloud is asked for only when the person joins a team (decided
   2026-10-28); the sign-in gate is gone (ORC-132). The start page says so in one line.
3. **In place, no modals.** Join, pick, link and download are steps of the start page: a breadcrumb
   (*Start › Join a team › Acme › Kontur*), a step line (*Join the team, Choose a project, Link a folder, Download
   tasks*) and *Back*.
4. **Join.** Orchester Cloud sign-in lists the account's team servers and invites (`docs/architecture/server.md`
   rows 43–44); *Join* gives this computer its own device sign-in on that server. A code works without Cloud: the
   server address and an 8-character invite or pairing code (row 45).
5. **Projects.** A team lists only the projects the person is invited to (row 50). Before any team is joined, the
   third way goes through Join and then shows the projects.
6. **Folder.** An existing checkout: `origin` is compared with the project's remote hint; a mismatch warns and *Link
   anyway* decides (row 49). Without one, *Clone repository…* clones it in a console window the person sees, and a
   clone that worked fills the folder field (choice A, revised by ORC-584).
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

- **A.** Revised by ORC-584 (the person, 2026-10-31): *Clone it here* clones in a console window the person sees,
  not in the app. ORC-290 and ORC-296, which cloned in a hidden process with progress in the app, stay canceled: there
  is no in-app progress bar and no in-app *Cancel*; closing the console cancels. (Decided on 2026-10-30, before this:
  no clone at all, only the `git clone` command with *Copy*.) Settings › Orchester's invite landing still shows the
  command with *Copy* (choice C).
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
- Decisions 5–6 as built (`crates/desktop-app/src/screens/join_team/shared/`):
  - **The way there.** *See your projects there* after a join and *Open* on a joined team lead to the projects
    (`/join-team?way=projects&server=<id>`), then *Link a folder* (`?way=folder&server=<id>&project=<slug>`).
  - **No task count.** The rows of 05 `v5qbWM` draw *212 tasks*; `GET /projects` has no count of a project's tasks and
    `next_number` counts numbers given, so the app shows none until the API has one.
  - **Clone.** The panel in the place of *Clone it here* holds no radio: its title, what it does, *Clone into* with
    *Choose…*, what Git will do with that folder and *Clone repository…* (see "Clone in a console" below). A project
    with no hint, or one an address can't hold safely (`clone_url`), has no panel and the words do not point to one.
  - **Already linked.** *Continue with Kontur* opens a project a checkout on this computer is linked to, past *Link a
    folder*. No frame draws this.
- Decisions 1–3 as built (`crates/desktop-app/src/screens/all_projects/welcome.rs` and `welcome/`):
  - **Choose folder….** It picks a folder: one that keeps a project opens it, a git repository Orchester has not set
    up (or a folder in none) opens Add project on it, and any other refusal says why under the ways in All projects'
    error line (`FV5pQ`), which no startup frame draws.
  - **The third way.** Signed in, it names the first team of the account this computer joined, how many of its
    projects are the person's once read (none is said when it can't be read), and leads to them; the invites waiting
    show as a badge. Before a team is joined it leads to *Join a team* (choice B as proposed).
  - **Signed out.** A fresh install starts signed out (ORC-132), so it shows 01 as drawn: *Sign in* on the second way,
    which opens *Join a team* on Orchester Cloud, and the line that no account is needed.
- Decision 9 as built (`crates/desktop-app/src/screens/all_projects/agent_cli.rs`):
  - **Where.** The callout `SIbdo` and fix row `U4QvYO` of frame 14 `mruEH` sit under the first start's ways (01
    `H9CZ1`) and above All projects' list, 1040 and 1120 wide.
  - **A first start has no connection to fix.** The chosen file is kept in the application database (`ui_state`,
    `orchester-app.agentProgram.<program>`) and offered in Add connection.
  - **What counts as not found.** With listed projects, a CLI connection's program file is gone and the search path has
    no copy of that CLI. With none, neither CLI is on the search path and no chosen file exists; it then names `claude`.
    A connection whose program is gone while a copy is on the search path shows nothing, since "not found" would be
    false.
- Decision 7 as built (`crates/desktop-app/src/screens/first_download/`, `/first-download?project=<id>`):
  - **Where it leads from.** A link of a checkout without tasks, on Project settings' link page, opens it. The steps
    before it (05, 06, 07) lead to the same route when they land.
  - **Team settings.** The sync does not read them yet (`DownloadProgress::team_settings`), so their row says *not
    downloaded yet* where frame 08 draws *after the tasks*, which would promise work that does not happen.
  - **The bar.** It fills to what was stored of what the server said (84 of 212 is 40 %; the frame draws 250 of 520),
    and there is none until the server said.
  - **Not drawn.** Frame 08 draws only the download under way. A failure shows in the tasks' row, with the reason and
    *Try again*, and the callout leaves; the end says *Tasks downloaded* without the callout and *Stay here*. *Stay
    here* puts the callout away and stays on the page. The tasks and the team settings are the only parts: the tags and
    modules the download reports are not drawn.

## Clone in a console (ORC-584, as built)

Code: `crates/exec/src/clone_console/` (the console, behind `cfg`), `crates/workspace/src/sharing/project/clone/` (the
destination check, the clone's end, `Clones`), `crates/desktop-app/src/screens/join_team/shared/link/clone.rs` and
`clone_view.rs` (the panel).

**No frame draws it** (the person, 2026-10-31: the design is not updated for ORC-584). 06 `go2aM` (and 13 `sN8Pa`,
13b `tDFeg`) still draw *Clone it here* as a collapsed radio option, and 07 `phoYc` the in-app clone with progress and
*Cancel*, which is not built. The clone panel in their place follows the panel of the checkout option above it
(`surface_panel`, `border`, `radius_lg`, its folder row as `d3Cx27` with the error under the row as `DNRz3`) and the
`Callout/*` tones: an agreed deviation, not a mismatch.

- **The flow.** *Clone into* is the folder the checkout gets; *Choose…* picks the folder it goes in and adds the
  repository's name (`kontur`). Under the row, what Git will do: *Creates the folder kontur in …*, or *An empty
  folder: Git clones into it*. *Clone repository…* (a person's press only, as linking is) opens a console window that
  runs `git clone -- https://<hint>.git <folder>`. While it runs the button is *Cloning in a console…* and an
  information callout says to sign in there if Git asks and that closing the window stops the clone. Leaving the page
  and coming back follows the same console; a second press opens no second one.
- **Proven done, never linked by itself.** A clone counts only when Git ended with 0 **and** the folder checks as the
  project's checkout (`check_folder` says it matches). It then fills the folder field, so the page's own check shows
  it matches and *Link and download tasks* goes on as for any folder. Nothing is linked until the person presses it.
- **Nothing is deleted.** A folder that already holds anything, a hidden file or a file at the path included, is
  refused under the row before any console opens (*This folder already has files…*), as are a missing parent, a path
  that is not absolute and one on another machine. What a failed or closed clone left is named (*Git left the folder
  …*) and kept; the check under the row then refuses it, so the next clone goes elsewhere.
- **The states.** Git not installed (none on the search path that starts directly and answers `git --version`, which
  also covers Apple's stand-in `/usr/bin/git`), the console did not open, Terminal did not start the script (macOS,
  within two minutes), Git failed with its code (a refused sign-in is one: Git's message stays in the console, not
  read by Orchester), the console closed before Git ended, Git ended with 0 but the folder is not the project's
  checkout, and Orchester lost track of the console. Each one leaves *Clone repository…* to press again.
- **`gh` is not needed.** The address is `https://`, with Git's own credentials: Git Credential Manager, the macOS
  keychain, or `gh` when the person set it up as Git's helper (`gh auth setup-git`). Pushing already works that way
  (`GIT_TERMINAL_PROMPT=0` in Orchester's own Git work), and `gh` is only for the pull request API. The console
  leaves `GIT_TERMINAL_PROMPT` and `GCM_INTERACTIVE` unset, so Git may ask there.
- **Values as data.** On Windows the console is `powershell.exe` from the system folder with `-NoProfile` and a fixed
  script; git's path, the address, the folder and the window's words reach it only as environment variables. On macOS
  Terminal runs a `.command` script of Orchester's own where each value is one single-quoted word. The window's words
  have no control characters. The address holds no user info and no shell characters (`clone_url`).
- **Platforms.** Windows: a PowerShell console of its own; after a failure it waits for Enter so Git's message stays
  readable. macOS: a Terminal window; whether it still runs is told by the process id the script writes first.
  Elsewhere: *not available on this system yet*. The console outlives the app if the app quits; a folder it left
  then shows as already having files.
