# Roadmap and releases

Status: draft for the person's review (2026-10-28). Nothing here is built yet.

Planning in Orchester above single tasks: which work ships in which release, when, and how far along it is. Today a
project has modules (epics, milestones, sprints) with start and target dates, a burndown and a delivery forecast
(`get_module`, `get_delivery`, the Delivery view), and tasks with story points and dependencies. What is missing is
a view of all that work over time, and the thing that ships: a release with a name, a date, its tasks and its
changelog.

| | Answers | Example |
| --- | --- | --- |
| Module | What we work on (a theme, an epic) | *Docs*, *Browser* |
| **Release** | **When it ships, and as which version** | *0.4.0 · Nov 15* |
| **Roadmap** | **Both over time, or by horizon** | modules as bars with releases as markers; Now / Next / Later |

## Decisions

1. **A release is its own level, not a kind of module** (the person, 2026-10-28). A task belongs to at most one
   module and at most one release, as a fix version. A module's work may ship across several releases (*Docs* D1 in
   0.4, D2 in 0.5), and a release gathers tasks of several modules.
2. **Orchester records a release; a person ships it** (the person, 2026-10-28). Orchester never bumps a version,
   tags, builds or publishes. The person ships the release their own way (for Orchester itself: the version bump in
   `crates/desktop-app/Cargo.toml` that CI releases, `docs/stable-build.md` › *Releases from CI*) and presses *Mark
   released*, which records the day and, when given, the commit. There is no release task or Release pipeline.
3. **Orchester composes the changelog; the person edits it** (the person, 2026-10-28). The draft is built without an
   agent from the release's Done tasks, grouped by module (*Other* for tasks without one), a line per task with its
   title and key linking to it. The person edits it in place on the release's page; new Done tasks are added to the
   draft without losing the person's edits. *Mark released* freezes it. It lives in the release's record, never in
   the repository, with *Copy as Markdown* (and *Save as doc* once Docs exist).
4. **Unfinished tasks move to the next release on their own** (the person, 2026-10-28). *Mark released* keeps only
   the release's Done tasks and moves every other one (Canceled aside) to the next planned release (see *Releases*
   for which one). With no such release they are left without a release. The page then says what moved and where
   (*3 tasks moved to 0.5.0*, each listed), and each move is kept in the task's history. A release whose date passed
   moves nothing: it reads *N days overdue* beside its forecast.
5. **A release's name is free text, with the next semantic version offered** (the person, 2026-10-28). The name is
   unique in the project (`0.4.0`, `2026.11`, `Beta`). When the latest release's name is a semantic version, *New
   release* offers the next patch, minor and major as one-click choices. Releases are ordered by planned date, not
   by name; this order decides the *next planned release* of decision 4.
6. **Roadmap has two views: Timeline and Now / Next / Later** (the person, 2026-10-28). Timeline lays the open
   modules over time with the releases as markers; Now / Next / Later sorts them by a horizon the person sets. See
   *The screens*.
7. **Releases and the roadmap work in shared projects from the start** (the person, 2026-10-28). A shared project's
   releases, each task's release and each module's horizon belong to its team server, as its modules do (ORC-78,
   `docs/architecture/server.md` › *Modules*). Every member sees the same releases and roadmap.
8. ***Roadmap* sits in the sidebar right under *Docs*** (the person, 2026-10-28).
9. **A planned release can be deleted, after a warning** (the person, 2026-10-28), in a modal (decision 15). See
   *Releases*.
10. **A released release is final** (the person, 2026-10-28): it is never reopened; its tasks, changelog and
    released day stay as *Mark released* left them.
11. **The commit on *Mark released* is picked or typed** (the person, 2026-10-28): a picker of the base branch's
    recent commits, or a hash typed in, checked against the repository either way.
12. **Agents in runs read releases and the roadmap** (the person, 2026-10-28) through Orchester's tools for agents.
    See *Agents and automation*.
13. **A module's horizon is set by a person, not derived from its dates** (the person, 2026-10-28). A new module
    starts in *Later*.
14. ***Modules* becomes a tab of Roadmap** (the person, 2026-10-28) and leaves the sidebar. See *The screens*.
15. **Confirmations are modals** (the person, 2026-10-28): *Mark released* and *Delete release* open a modal dialog
    over the page, not a panel or warning inserted into it, because a state that appears in the layout makes the page
    jump. Creating a release stays a page. The rule is now app-wide (`.claude/skills/orchester-ui/SKILL.md` ›
    *Confirmations and approvals in a modal*).

## Releases

- A release has a name (decision 5), a description (optional, as a module's), a planned date (optional, a whole
  local day), a state *Planned* or *Released*, and once released the day it was released, who marked it and the
  commit (optional).
- **Order.** Planned releases with a date come first by date, then those without one in the order they were
  created. The *next planned release* of decision 4 is the first planned release after this one in that order,
  this one left out.
- **Progress and forecast** are a module's: scope and done in tasks until the tasks have story points, Canceled out of
  scope; a burndown by day from the release's first task to its planned date; the 50 % and 85 % dates of the
  project's Monte Carlo (`get_delivery`), which forecasts each planned release beside each open module. A planned
  date before the 85 % date reads *Likely Nov 20 · after planned date* in `warning`, as a module's target does.
- **Mark released** opens a modal over the release's page (decision 15): it lists the Done tasks that ship,
  the unfinished ones and the release they will move to (decision 4), and an optional commit (decision 11: a picker
  of the base branch's latest commits, newest first with their subject and day, or a hash typed in; either is
  checked to be a commit of the repository). *Mark released* in the dialog records it; *Cancel* or Esc closes it.
  It cannot be undone (decision 10), and the dialog says so.
- **Changelog** (decision 3) is a list of entries, not free text, so new tasks join it without disturbing edits:
  - a task entry for each Done task of the release, its text the task's title until the person edits it, under its
    module's heading; the person may edit its text, hide it, or move it within its group;
  - free entries the person adds (*Add line*), and an optional intro above the groups;
  - a task that leaves the release, or is no longer Done, takes its entry out (an edited text is kept with the task
    and comes back if it returns).
  *Mark released* freezes the entries as they are. *Copy as Markdown* renders the intro, a `###` heading per module
  and a `-` line per entry ending in the task's key.
- **Delete** (decision 9), for a planned release only: `…` › *Delete release* opens a modal (decision 15): *Delete
  0.4.0? Its 7 tasks are left without a release and its changelog edits are lost. This cannot be undone.* with
  *Cancel* and *Delete release* (`danger`). Deleting takes its tasks out (each a
  move by the person in `release_changes`, so burndowns and task histories keep it) and removes the release and its
  changelog. A released release has no delete.

## The screens

Pencil designs come first (on the *Orchester design* pipeline), in the desktop file
`C:/Users/lurkm/.pencil/documents/87147a2b-89b6-4bf9-ac60-a41596df72da/orchester.pen`, using the Modules pages'
geometry (`Modules · redesign` `oEELJ`, the task page's header). Confirmations are modals, new things are pages
(decision 15). Rough wireframes for discussion (not a design to build from) are on the board `Roadmap ·
wireframes` `HtxlM` of that file: 01 Timeline, 02 Now / Next / Later, 03 Releases, 03b Modules, 04 release Summary,
05 Mark released (modal), 06 Changelog, 07 released with tasks moved, 08 Delete (modal), 09 New release, 10 a task's
Release.

- **Roadmap** (`/project/<id>/roadmap?view=timeline|horizons|modules|releases`), a sidebar entry right under *Docs*
  (decision 8) with the views *Timeline*, *Now / Next / Later*, *Modules* and *Releases* as its tabs, and *New
  release* and *New module* in its header.
- **Modules** (decision 14): the sidebar entry *Modules* goes; the Modules list as built (ORC-112: search, Open ·
  Closed, a row per module) becomes the *Modules* tab, its Open · Closed views a switch under the tabs. A module's
  page and its new and edit pages keep their addresses and layout; their back row reads `← Roadmap / name` and
  returns to the tab. `/project/<id>/modules` opens the tab, and MCP `navigate` `modules` keeps working.
- **Timeline** (`view=timeline`, `&scale=month|quarter`):
  - a row per open module, ordered by start date, its name and *N of M done* at the left; its bar runs from its
    start (or its first task's day) to its target date; without a target it runs to the 85 % date, dashed;
  - on the bar the forecast: the 50 % date as a tick, the 85 % as a whisker, both in `warning` past the target;
  - releases as vertical markers across the rows with their name at the top: dashed while planned (in `warning` when
    overdue), solid in `success` once released; a marker opens its release;
  - a line for today, *Today* to scroll to it, *Months* · *Quarters*; horizontal scroll and wheel with Shift;
  - modules without dates and without forecast in a last row *No dates*, as chips; a module opens its page;
  - *Show closed modules* adds closed ones, muted. Dates are not dragged on the timeline; they change on the
    module's page.
- **Now / Next / Later** (`view=horizons`): three columns of the open modules by their horizon, each card with the
  name, the meter, the forecast and the releases its tasks are in (chips). Dragging a card between columns or within
  one sets its horizon and its place; the card's `…` has *Move to Now / Next / Later* for the keyboard. A new module
  starts in *Later*; a closed module leaves the board and keeps its horizon. Dates never change a horizon
  (decision 13).
- **Releases** (`view=releases`): Planned, then Released (newest first), a row each in the Modules list's look: the
  name, the planned or released day, the forecast for a planned one, a meter and *N of M tasks done*. An empty
  state with *New release*.
- **Release page** (`/project/<id>/releases/<release id>`, tabs Summary · Changelog · Burndown): `← Roadmap / 0.4.0`,
  *New task in release* and `…` (*Edit*, *Mark released*, *Delete release*; a released one has none of them); the
  status line (state chip · planned date · progress ·
  forecast or *N days overdue*, or *Released Nov 14 · 3f2a9c1*).
  - *Summary*: the description; the tasks grouped by module, one-line rows as a module's Summary; the properties
    column with State, Planned date (picked with `ui::date_picker`), Released, Created; once released, the notice
    of what moved where.
  - *Changelog*: the entries edited in place (decision 3) with *Add line* and *Copy as Markdown*; read-only once
    released.
  - *Burndown*: the module's burndown card over the release's tasks.
- **New release / Edit release** (`/releases/new`, `/releases/<id>/edit`): pages in the module form's style; the name
  with the next patch / minor / major offered as chips when the latest release's name is a semantic version
  (decision 5), the description, the planned date.
- **Task.** The task form and the task page's properties get *Release* beside *Module* (a picker of planned
  releases; a released one shows read-only). Tasks' Filter and the board's Display options take *Release* (columns
  and rows by release), as they take *Module*.
- **Module page.** Its properties column gets *Horizon* (Now, Next, Later), and its Summary the releases its tasks are
  in.

## Data and storage

Beside the modules' tables in the project database, with the same shape (`docs/architecture/storage.md`):

| Table | Purpose |
| --- | --- |
| `releases` | id (UUIDv7), name (unique without regard to case), description, planned date, state, released on, released by, released commit, revision (shared projects; empty for a local one), created at and by |
| `task_releases` | task, release; at most one per task |
| `release_changes` | every move of a task into a release, out of one or from one to another: sequence, task, from, to, by, actor (`person` or `orchester`: decision 4's moves are Orchester's), at; append-only, read by the burndown and the task's history |
| `release_changelog` | release, entry id, kind (task or line), task, text (null while it follows the task's title), hidden, group, position; the intro on the release |
| `module_horizons` | module, horizon (`now`, `next`, `later`), position within the horizon |

- Core: `orchester_core::release` (the release's details with their invariants, the order and the next planned
  release, the move on *Mark released*, changelog composition and its Markdown), beside `orchester_core::module`.
  The horizon is part of a module's details.
- Shared projects (decision 7), as `server.md` › *Modules* does it: a `Release` shape with its revision; a task's
  `release` as a field of `Task`, `TaskContent` and `Changes`, moved under the task's revision; `ReleaseChange`
  history; the changelog entries and *Mark released* as release changes under its revision (a mismatch refuses, as
  a module change does); a module's `horizon` and position in `Module`. A `releases` event like `modules`; Share,
  Join and Make local carry releases as they carry modules. Progress, burndown and forecasts are not served: each
  desktop computes them. The API contract (`orchester-api`, `docs/cloud-api.md`) changes here first; the server's
  half lives in the private `orchester-platform` repository.

## Agents and automation

- Orchester's tools for agents (`orchester_tools`), offered to every step (decision 12): `list_releases`,
  `get_release` (details, tasks, progress, forecast, changelog) and `get_roadmap` (open modules with their dates,
  horizon, progress and forecast, and the releases in order). Reading runs without asking. A run's own task tells
  its release, so an agent knows which version its work ships in. Agents do not change releases or horizons through
  these tools.
- The app's MCP server: `list_releases`, `get_release` (details, progress, tasks, changelog, burndown, forecast, as
  `get_module`), `create_release`, `update_release` (details and changelog entries), `mark_released {commit?}`
  (reports what moved where), `delete_release` (a planned one; it reports the tasks left without a release);
  `create_task` and `update_task` take `release`; `update_module` takes `horizon` and
  its position; `get_delivery` forecasts planned releases; `navigate` takes `roadmap` (`view`, `scale`) and
  `release` (new, or an id with `tab` or `view` edit).
- None of these widens what agents may do; they follow the screens' rules and refusals.

## Phases

1. **R1 · Core and storage (non-UI).** Release types and rules, a task's release and its history, changelog entries,
   the module's horizon, tables and migrations, the forecast for releases, the workspace's service through
   `TaskWriter`. Tests in `orchester-core`, `orchester-store`, `orchester-workspace`.
2. **R2 · Shared projects.** The API contract here, then the server's half in `orchester-platform`, and the desktop's
   cache and writes, as ORC-78 did for modules. Before R3 ships, so releases work in shared projects from the start.
3. **R3 · Releases (UI).** Releases view, release page with Summary, Changelog and Burndown, new and edit pages,
   *Mark released* and *Delete release* modals, *Release* on the task form, task page, Filter and board. Needs a Pencil design first.
4. **R4 · Roadmap (UI).** The Roadmap page under *Docs*, Timeline and Now / Next / Later, the Modules list moved into
   its *Modules* tab (the sidebar entry removed), *Horizon* on the module page. Needs a Pencil design first.
5. **R5 · Agents and MCP.** The agent tools and the app's MCP tools above.
6. **Later:** *Save as doc* for a changelog (after Docs), dragging dates on the timeline.

## Open questions for review

None left; the draft waits for the person's review.
