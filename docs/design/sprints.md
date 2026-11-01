# Sprints

Status: design for the person's review (2026-10-30). Nothing here is built yet. Pencil: board `Sprints · redesign`
`zIusc` in `crates/desktop-app/design/orchester.pen`.

Planning in time boxes: a week (or two) of the project's work with a goal, what was committed and how it moved, a
burndown, and once it closes a report and a retro. It builds on what exists: modules and their burndown (ORC-48),
story points and their calibration (ORC-49), delivery charts and the Monte Carlo forecast (ORC-50), the daily report
(ORC-47) and Usage. It sits beside the planned releases and roadmap (`docs/design/roadmap.md`).

| | Answers | Example |
| --- | --- | --- |
| Module | What we work on | *Webhooks v2* |
| Release | What ships, as which version | *0.4.0 · Nov 15* |
| **Sprint** | **When we work on it** | *Sprint 14 · Oct 27 – Nov 2* |

## Decisions

The person accepted the proposal of 2026-10-30 with its recommendations ("норм").

1. **A sprint is its own level, not a kind of module.** A task is in at most one module and, at a time, in at most
   one sprint; the two are independent. Modules stay epics and themes. Every move of a task into or out of a sprint
   is kept with who made it (a person, or Orchester on Close sprint).
2. **One cadence per project.** Project settings › General › Sprints: *Plan in sprints* (off by default), *Length*
   1 week (default) or 2 weeks, *Starts on* a weekday (Monday by default). Changes apply from the next sprint.
3. **One active sprint per project**, in shared projects too. *Make the next sprint ahead* (on by default) makes the
   next sprint, empty and planned, when one starts, so tasks can be planned into it.
4. **The commitment is what the sprint holds at the end of its first local day.** Later moves into it count as
   added, moves out as removed; tasks moved in by the previous sprint's close count as carried in.
5. **Close sprint is a modal** (app rule of 2026-10-28) from the sprint page's `…`, Overview's last-day card or
   `close_sprint`: Done tasks stay; unfinished ones move to the next sprint (default; Project settings ›
   *Unfinished tasks*) or out of sprints; Canceled ones stay, out of scope. A closed sprint is final: never
   reopened, its report and retro keep what they showed.
6. **Counted in tasks, with story points beside them.** Done is the task's workflow Done, as the burndown counts it;
   merged is shown apart where it differs. Velocity in points comes when the estimator's sizes spread (today 57% of
   Orchester's merged tasks are 13 points).
7. **Effort is tokens and active agent time, never money** (`docs/architecture/native-agent.md` decision 8). In a
   shared project the team server keeps no usage (`docs/architecture/server.md`, *What the server never stores
   about runs*), so a sprint's tokens and agent time are this computer's runs and the report says so.
8. **Facts are read, never guessed.** The report and the retro's facts come from the project's data; the person
   writes the retro notes; the Orchestrator drafts notes only when the person presses *Draft notes with the
   Orchestrator*, and only proposes them. An action in the retro becomes a task.
9. **Sprints are a tab of Roadmap.** The sidebar's *Modules* becomes *Roadmap* (`roadmap.md` decision 14). If
   sprints ship before the Timeline, Roadmap starts with only its *Sprints* and *Modules* tabs.

## What is counted

All by local days, from the project's store; a sprint's days run from its start through its end.

| Measure | Rule |
| --- | --- |
| Committed | Tasks in the sprint at the end of its first day, Canceled left out |
| Added, removed | Moves into and out of the sprint after the first day, until it closes |
| Carried in, carried over | Moves made by the previous sprint's close; moves this sprint's close made |
| Done | Tasks in the sprint whose workflow status is Done (at close, or now) |
| Done of committed | Committed tasks Done by the close, over Committed |
| Velocity | Done tasks (and their points) per closed sprint; the average of the last three |
| Cycle and lead time | As Delivery's (first agent start to merge; creation to merge), over the sprint's Done tasks |
| Agent time and tokens | Turns completed on the sprint's days, of runs whose task was in the sprint at that moment, as Usage counts them |
| Waited for you | Time the sprint's runs waited for the person (tool requests, questions, plans, messages) on its days |
| Where agent time went | Delivery's per-step agent time, executions and returns, over the sprint's tasks and days |
| Points and effort | Calibration's medians per points value, over the sprint's merged tasks |
| Likely done | Delivery's Monte Carlo over the project's last 30 days, from the remaining work |
| Likely holds | The same history run the other way: how many tasks a sprint's days deliver, at 50% and 85% (at least) |

While a sprint is active its numbers are read live. At close a snapshot of its report is kept, so a closed sprint
reads the same later even if run records are removed or estimates change.

## The screens

Board `Sprints · redesign` `zIusc`; every page uses the module page's geometry (`Modules · redesign` `oEELJ`, the
task header `j8DakG`).

- **01 · Roadmap · Sprints** `bMDxS` (`/project/<id>/roadmap?view=sprints`): header *Roadmap* with *Search sprints*
  and *New sprint*, the cadence in the status line (*Cadence* opens Project settings › General), the tabs Timeline,
  Now / Next / Later, Sprints, Modules, Releases. Body: the active sprint (goal, meter, committed / added / removed,
  the 50% and 85% dates, in `warning` past the end); *Velocity* (per sprint, held and done, the current sprint as
  done so far; *Tasks* · *Points*; the average and what the next sprint likely holds); *Planned* and *Closed* rows in
  the Modules list's look.
- **02 · Sprint · Summary** `XHn6Z` (`/project/<id>/sprints/<id>`): `← Roadmap / Sprint 14`, *New task in sprint*,
  `…` (*Edit*, *Close sprint*); the status line (state chip · dates · day N of M · done · forecast); tabs Summary ·
  Burndown · Report · Retro. Summary: the goal; the commitment strip (committed at the end of day 1, added,
  removed, carried in); tasks by workflow status with *carried ×N* and *added <day>* marks, module and points; the
  burndown line; properties State, Start, End, Cadence, Likely holds, Next, Created.
- **03 · Sprint · Burndown** `rvVlP`: the module's burndown card over the sprint's days with *Burndown* · *Burnup*
  (Burnup draws scope and done, so added work shows), and *Scope changes* below: day, added or removed, task, who.
- **04 · Sprint · Report** `b2W60C`, a closed sprint: the numbers strip (done of committed, scope after day 1,
  carried over, cycle and lead time with the change from the sprint before, agent time, tokens, waited for you);
  *Delivered by module*, *Carried over* with why, *Most effort*; *Where agent time went*, *Story points and
  effort*. *Copy as Markdown* in the header. An active sprint's Report reads the same, so far.
- **05 · Sprint · Retro** `BT3Ff`: *What the sprint shows* (facts with their numbers and tasks, good or a warning),
  *Draft notes with the Orchestrator*; *Notes* edited in place (*Went well*, *To improve*, *Actions*; an action
  links its task or offers *Create task*).
- **06 · New sprint · planning** `sSYZe` (`/sprints/new`, a page): name (next number), goal, Start and End chips from
  the cadence; *Tasks to take in*: the capacity bar (taken in against the 50% / 85% band, *Fits* or *More than
  usually delivered* in `warning`), what comes over from the active sprint when it closes, Todo and Backlog with
  checkboxes and *Take in all*. *Create sprint*. Editing a planned sprint is the same page.
- **07 · Close sprint · in a modal** `OM3fi`: what stays, the unfinished tasks with the sprint they move to (a
  select: the next sprint, or *No sprint*), canceled ones, that closing is final; *Open the retro after closing*.
- **08 · Project settings · General · Sprints** `cPSdJ`: the cadence rows of decisions 2, 3 and 5, and *Sprint
  review*.
- **Parts** `SYdws` Overview's sprint card (and its last-day form with *Close sprint*); `x7ZdAm` the daily report's
  Sprint section and the last day's review line; `PuELl` the task page's Sprint property with the sprints it was in
  before, and the task form's Sprint chip and menu (active and planned sprints, *No sprint*); `q4N8iP` Tasks'
  Filter › Sprint and Display › Group by Sprint; `X0Qn6V` states: no sprints yet, velocity before the first close,
  not enough history, a shared project's usage note. Notes `esBrO`, open choices `TMgR6`.

## Data and storage

Beside the modules' tables in the project database (`docs/architecture/storage.md`):

| Table | Purpose |
| --- | --- |
| `sprints` | id (UUIDv7), number, name (unique without regard to case), goal, start and end day, state (`planned`, `active`, `closed`), closed at and by, revision (shared projects), created at and by |
| `task_sprints` | task, sprint; at most one per task |
| `sprint_changes` | every move of a task into a sprint, out of one or from one to another: sequence, task, from, to, by, actor (`person` or `orchester`), cause (`moved`, `closed`), at; append-only |
| `sprint_reports` | the report snapshot kept at close |
| `sprint_retros` | the notes by section and the actions with their task |

The cadence is part of the project's settings. Core: `orchester_core::sprint` (details and their invariants, the
cadence and the next sprint's days, commitment, carry-over and close), with the burndown's tracks shared by
modules, releases and sprints. `workspace::delivery::forecast` gains *how many in N days* beside *when*.

## Shared projects

As `server.md` › *Modules* does it: a shared project's sprints, each task's sprint, the moves and the retro belong
to its team server, under revisions; *Close sprint* is a sprint change under its revision, and its moves are
Orchester's. Progress, burndown, forecasts and the report are computed on each desktop (row 69). Usage stays
local: tokens and agent time are this computer's runs (decision 7). The API contract (`orchester-api`,
`docs/cloud-api.md`) changes here first; the server's half lives in `orchester-platform`.

## Agents and automation

- The app's MCP server: `list_sprints`, `get_sprint` (details, commitment, tasks, burndown, forecast),
  `get_sprint_report` (the report's numbers, as the Report tab), `create_sprint`, `update_sprint`, `start_sprint`,
  `close_sprint {unfinished_to}` (reports what moved where); `create_task` and `update_task` take `sprint`;
  `get_delivery` and `get_usage` take `sprint`, and `get_usage` takes `module`; `navigate` takes `roadmap`
  (`view=sprints`) and `sprint` (new, or an id with `tab`).
- Orchester's tools for agents read the run's task's sprint and its goal; agents do not change sprints.
- The daily report gets a Sprint section while a sprint is active, and on its last day the review (Project settings
  › *Sprint review*). The Orchestrator may propose a next sprint's scope or retro notes; a person applies them.

## Phases

1. **S1 · Core and storage.** Sprint types and rules, the cadence, membership and its history, commitment,
   carry-over and close, the shared burndown tracks, *how many* forecast; tables and migrations.
2. **S2 · Shared projects.** The API contract here, the server's half in `orchester-platform`, the desktop's cache
   and writes.
3. **S3 · Workspace and MCP.** Sprint reads and writes through `TaskWriter`, the report and its snapshot, the MCP
   tools, `get_delivery` / `get_usage` filters, the daily report's Sprint section and review.
4. **S4 · UI: Roadmap and sprint pages.** Sidebar *Roadmap* with the Sprints and Modules tabs, the sprint page's
   Summary and Burndown, New sprint, Close sprint, the settings group, the task's property and chip, Tasks' Filter
   and Display, Overview's card.
5. **S5 · UI: Report and Retro.** The Report and Retro tabs, *Copy as Markdown*, *Draft notes with the Orchestrator*.
6. **S6 · Usage.** Sprint and Module filters on the Usage page.

Each UI phase is checked against the board by a separate verifier (`.claude/skills/design-verification`).

## Open choices

- Retro notes in a shared project: on the team server for every member, or kept on each computer.
- Velocity in story points by default once the estimator's sizes spread out.
- A custom length in days besides 1 and 2 weeks.
- A *Goal met* / *not met* mark when a sprint closes.
