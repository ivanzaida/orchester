# Team authority: the desktop's claims, member views, module branches and outage states (ORC-693)

Status: draft for the person's review (2026-11-04). Nothing here is built yet. It draws the desktop half of the team
server's authority (milestones P6 and P7 of `docs/architecture/team-authority-plan.md`; the platform contract
`docs/server-authority.md` on `codex/team-server-authority`; `orchester-api` v2 on master: `AuthorityError`,
`DecisionId`, `WorkspacePolicy`, `WorkspaceCapabilities`). The team server holds team tasks, modules, tags and
configuration; the desktop reads them from it and keeps no copy. Only workspace Admins (and Server admins, who
inherit it) change them; members read, run and claim.

## Where it is

File: the desktop design `crates/desktop-app/design/orchester.pen`, board **Team authority · redesign** `BZc66`
(x 136580, y -561, right of *Settings consolidation · redesign*, like the other boards added after the Read me's
list). Every window is an instance of `Window/Project` (`qlXhJ`) copied from an existing redesign frame: the Tasks
list *01 · Active* (`DQozX`), the shared board *14 · Shared project* (`ZLX5M`), the Task page *01 · Summary*
(`AsE7R`), the Modules list *01 · Modules* (`m63AnV`), the Module page *02 · Module · Summary* (`KpLYv`) and
Project settings *04b · Sharing · shared* (`TnUGj`). The member pattern is the one of *Shared configuration ·
redesign* (`pjEjf`): no edit controls, one calm lock line naming the Admins. Reused components: `Button/*`,
`Notice/*`, `Callout/*`, `Field/Focus`, `Field/Error`, `Select/Default`, `Toggle/Switch off`, `Avatar/Person`,
`State/Empty`, `Task header`. No new variables or components. Exports at scale 1: `.tmp/design/ORC-693/<node id>.png`.

Fixtures: team Acme on `orchester.acme.dev/kontur`; Admin dana-k, Server admin olha, members ivan (the person, on
DESKTOP-IVAN, also DESKTOP-OFFICE) and mara (MARA-LAPTOP); task KON-12 in module Webhooks v2. All illustrative.

| Row | Node | Frames (dark) | Light twins |
| --- | --- | --- | --- |
| Head | `i0wzN` | title and lead | — |
| 1 · Member views | `rD8mt` | `QdL7N` 01 Tasks · member; `zv7b6` 01b Tasks board · member; `Tb55G` 01c Task · member · claimed by you on this computer; `rvXxg` 01d Modules · member; `FuHYN` 01e Module · member | `gN0kJ`: `IZzlk`, `SszZN`, `FWkHy`, `Bsza5`, `LpEgB` |
| 2 · Task claims | `EQY4d` | `nMUwg` 02 claimed by another member; `XbNXU` 02b claimed by you on another computer; `l82OS` 02c claimed under a module; `lpji3` 02d claim stale; `T68vo9` 02e take over a stale task (dialog); `DZ5u0` 02f take over refused; `u6gvz` 02g waits for you · claim not renewed (D1); `xUwYS` 02h claim line by state | `GWZmf`: `SBcw7`, `un4Ql`, `R3jibC`, `trQ2z`, `t3dVIa`, `x184D`, `W1pNe8`, `M7IIHj` |
| 3 · Authority lost | `ZQJmT` | `hWXxr` 03 taken over; `YQbnB` 03b membership removed; `W6Ncf` 03c claimed on your other computer | `VW3Km`: `Oj3Yn`, `A7yomH`, `a6g971` |
| 4 · Module | `d3hbfi` | `MvniR` 04 admin · branch, assignee and claim; `InYp8` 04b admin · change the branch (dialog); `E4Evt` 04c branch assignment by state; `PqVUn` 04d member · claim stale · take over refused; `GujCo` 04e admin · delivery awaits authorization; `Wd4G1` 04f module delivery by state | `W8kUP`: `puGjt`, `Yoz5E`, `Xr0bs`, `iazaU`, `uYOPv`, `q8tlb` |
| 5 · Revision conflicts | `AmoGD` | `LmxUY` 05 admin edit changed by someone else first; `H7MBjm` 05b revision conflicts by place | `u93QQZ`: `xKp6F`, `gZqxb` |
| 6 · Server outage | `eDNSQ` | `GR3RS` 06 Tasks · team data unavailable; `l6p9ra` 06b Task · run keeps going; `t8T8Kj` 06c finished here · delivery not accepted yet; `B7Mbe` 06d Module · unavailable; `I3CSI` 06e run outcome and team acceptance | `uBf6c`: `BgyG6`, `RHziI`, `Ujfel`, `yLcSM`, `a9j7UG` |
| 7 · Roles, Sharing, versions | `kYCyl` | `MnVch` 07 Sharing · admin; `FSeEz` 07b Sharing · member; `EERcN` 07c Members · admin; `BDJKM` 07d version refused; `BLTsP` 07e role labels and version refusals | `yTPyw`: `G9GmTV`, `Dp8bm`, `nN0HA`, `oGAQ6`, `gbpI4` |
| 8 · Notes | `wHO2s` | `FxkEC` what the design decides; `t43O34` open decisions | — |

Windows are 1440 × 900 like the Read me says; the sheets (02h, 04c, 04f, 05b, 06e, 07e) are 760 wide.

## States

### 1. Member views (01–01e)

- **Tasks list and board.** No New task, no quick add, no column add; cards don't drag. Under the header one lock
  line: *Only Admins change team tasks, modules and tags: dana-k (Admin) and olha (Server admin). You read, run and
  claim.* The board's team line reads *Team project on orchester.acme.dev/kontur · Read from the server · You're a
  member*; the old *Not shared* filter and the *was KON-…* refusal notice of the replica model are gone.
- **Task page.** No edit affordances on the properties; the primary action is the run's own (Pause run, Answer,
  Continue, Take over), never an edit. The lock line opens the Summary. The side panel gains *Assignee* and *Claim*,
  and *Merges into module/webhooks-v2* under the task's branch (D3).
- **Modules and Module page.** No New module, no New task in module, State not a picker. Each module row names who
  holds it (*olha*, *ivan (you)*, *Not claimed*). The Module page's side panel shows Assignee, Claim, Branch and
  Delivers to as text.

### 2. Task claim and authority (02–02h)

One line at the top of a team task's Summary says who holds the claim, on which computer, and when it was last
renewed. Neutral for a healthy claim, warning when it isn't renewed or is stale, danger when authority is lost.

- **Claimed by you on this computer** (01c): *DESKTOP-IVAN holds the claim and the running step renews it · renewed
  15:42 · stale after 15 min without renewal.*
- **Another member** (02) or **you on another computer** (02b): the run's steps and conversation are on that
  computer; the Run section says so and shows only what the server reports (*Running · last report 15:41*). Nothing
  to start here. A claim belongs to a computer, not to the account.
- **Under a module** (02c): *Claimed under module Webhooks v2 by olha*; a task can't be claimed on its own while
  the module claim holds, even when the task itself is idle.
- **Stale** (02d): warning line with the time since renewal and the server's threshold; the holder can still renew
  until someone takes over; *Take over* in the line and as the primary action.
- **Take over** (02e): a dialog says the claim moves to this computer, the former claim stops counting, and a new
  run starts from the Admin's pipeline with nothing of the former run.
- **Refused** (02f): *Couldn't take over KON-12 · mara renewed her claim at 15:43, before your request reached the
  team server*; the line shows her fresh claim; nothing started here.
- **Waits for you · D1** (02g): only Running work renews; the server refuses renewal for a waiting run (or one in
  review) with `decision_required` (D1, decided). The line says the claim isn't renewed and when it goes stale;
  decision ids stay in the frame captions, never in on-screen text.
- **Sheet** (02h): every case side by side, including *your claim is stale, nobody took it · Continue*, *nobody
  holds it · Claim* and the four authority-lost lines.

### 3. Authority lost (03–03c)

Taken over (03), membership removed (03b), or the claim moved to your other computer (03c). The run stops
(*Stopped · Taken over by mara*), its worktree stays on this computer as detached work (*Open worktree*), and its
unsent reports, refused by the server, stay readable and aren't sent again. When the person is removed from the
team, the server no longer shows the task: the page is the run's own record from its start, without the side panel.
An Admin's unassignment (02h) ends the claim at once in the same way.

### 4. Module page (04–04f)

- **Work and delivery** section under the description: the module claim, the branch and delivery to main.
- **Branch** (04, 04b, 04c): *module/webhooks-v2*, named automatically at first assignment, with *-<8 hex>* on a
  collision; task runs merge into it, only module delivery takes it to main. Admins see *Change*, which opens a
  dialog with the new name. Cases: edit; branch taken (field error); refused while a run of the module works
  (danger notice); revision conflict (05's pattern); a member sees it read-only.
- **Assignee and claim** (04, 04d): Assignee is a picker for Admins and text for members; Claim reads *olha ·
  renewed 15:40* or *olha · stale*. The line says child runs don't renew the module claim; what does is open (D1).
- **Takeover refused** (04d): *Couldn't take over Webhooks v2 · KON-12 is Running under olha's module claim*.
- **Delivery** (04e, 04f): not requested; requested (member: *waits for an Admin*); awaiting authorization (Admin:
  *Changes*, *Authorize* for the exact commits); authorized by an Admin; authorized automatically by Automatic merge
  (D7, decided); Automatic merge turned off before the delivery was reported, which fails it (*Ask again*).

### 5. Revision conflicts (05, 05b)

An Admin edit that started from an older revision is refused (`revision_conflict`). The place shows the current
value and who set it, says nothing of the edit was saved, and offers *Reapply* (send it again over the current
value) and *Keep* (drop it). Drawn for a task property (05, the changed field highlighted), the brief, a module's
target date, a tag rename, a board move between Backlog and Todo, and Automatic merge.

### 6. Server outage (06–06e)

- Team lists and pages say *Team data is unavailable* with the time and *Try again*; no old copy is shown as live,
  and Orchester keeps none (06, 06d). The Tasks list keeps one section, *Running on this computer*, with each run's
  waiting reports; the sidebar's *Running* lists the same runs (KON-12, KON-21).
- A Task page of a running run (06b, 06c) keeps the run (it uses its saved inputs) and drops the team fields from the
  side panel (*Team fields unavailable*), keeping only the run's branch and pipeline; the brief has no version
  picker, since its versions are team data.
- Local success apart from team acceptance (06c, 06e): *Run 8 succeeded on this computer* and *Delivery not accepted
  yet* are two lines. Outcomes: succeeded doesn't mark the task Done; failed keeps the claim; cancelled releases an
  independent claim; stopped at its limit stays resumable; reports refused for a lost claim stay readable.

### 7. Roles, Sharing and versions (07–07e)

- Role labels are **Member**, **Admin** and **Server admin** wherever a team person or the person's role shows
  (Your role, the members list, the lock lines); *Owner* is gone.
- **Sharing** (07, 07b): *Automatic merge* is a switch for Admins and text with a lock for members (default off);
  *Stale claim after 15 min* is read-only, set by the Server admin (5 min to 24 h). *Make this project local* is gone.
- **Members** (07c): Admin view with a role picker for members, a lock on the only Admin, and olha's Server admin
  coming from the server.
- **Version refused** (07d, 07e): *This team server needs Orchester 0.4.0* with *Check for updates*, or, the other
  way, *orchester.acme.dev needs an update* naming its Server admin. Both refuse; local projects work as before.

## Open decisions (drawn as open, no behaviour invented)

- **D1** — decided for tasks: only Running work renews, so a run waiting for a person or in review isn't renewed
  (drawn in 02g, 02h). Still open: what renews a module claim. Drawn: the module line only says its tasks' runs
  don't renew it; the frame captions and these notes carry the decision id, the on-screen text doesn't.
- **D4** — completing a task outside a run, manual blocked and reopen. Drawn: a succeeded run never marks the task
  Done; there is no Done action. **Decided** (the person, 2026-11-08, `docs/architecture/server.md` row 96): the person
  holding the task's claim or an Admin marks it Done; only an Admin cancels it, reopens a Done or Canceled task, or
  moves it between Backlog and Todo; a task becomes Done by itself when the server accepts the delivery of its work.
  Closing (Done or Canceled) releases the task's claim. Report and outside work, which has no base merge, is Done when
  its run succeeds. The manual Blocked is set and cleared by the claim's holder or an Admin. Not drawn or built yet
  (`docs/plan.md` S12).
- **D6** — default pipeline and the remaining field ownership. Drawn: the pipeline shown is the Admin's; members have
  no control to change it, and no new field ownership is drawn. **Decided for a task's pipeline** (the person,
  2026-11-05, `docs/architecture/server.md` row 80): a team task's pipeline is one of the team's pipelines, chosen by
  its resource id; only admins create and publish team pipelines, and a person's own local pipelines are not used for
  team tasks (hidden in a team project). Whoever may edit the task chooses it; the server's task route decides who
  that is (its Admins today). Still open: the default pipeline and the limits (refused as D6).
- **D8** — offline continuation, human waits and delivery after an outage. Drawn: running work goes on and reports
  wait; what the server does with a claim that went stale during the outage is named as open.

## Notes for the implementation

- The page's member/admin difference comes from the server's capabilities (`WorkspaceCapabilities`, the contract's
  `Capabilities.actions`), never from a cached role flag.
- The claim line, the module's *Work and delivery* section and the outcome lines are one strip pattern (icon, title,
  detail, optional actions) in four tones; the implementation can share one view for them.
- The sidebar instance carries the known warning (`dU03e`) of the shared `Sidebar v2`; it is not from this design.
