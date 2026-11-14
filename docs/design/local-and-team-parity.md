# Local and team projects behave the same

Status: decided by the person on 2026-11-13. The table below is what the code did on that day (`origin/master`
`6a73865ae` with the Orchestrator autonomy work of `docs/plan/orchestrator-autonomy/`), read from the code and not run
against a team server.

## The rule

A project on this computer only and a project linked to a team server are one product. What a person, an agent or
the Orchestrator can do in one, they can do in the other, the same way, with the same words. This holds for
everything Orchester does without a person too: leading tasks and modules, merging, continuing, answering.

- A feature is built for both kinds, in the same change, or it says here what is missing for the other and which
  task closes it.
- A refusal that names the kind of project (`team_project_unsupported` and its like) is a gap with a row in the table
  below, never an accepted limit.
- A plan or a design does not scope itself to one kind. "Local projects only" in an older plan
  (`docs/plan/pipelines-v2/README.md`) is how that work was cut, not the rule; what it left different is in the
  table. `AGENTS.md` names this page as the rule.

## What differs by nature, and stays

These are not behaviours but who owns what. They explain rows of the table; they are not gaps.

| | Local project | Team project |
| --- | --- | --- |
| Tasks, modules, tags, team configuration | kept on this computer, changed by the person | kept on the team server; a workspace Admin changes them, members read, run and claim (`docs/design/team-authority.md`) |
| Runs, worktrees, conversations, secrets, connections | this computer | this computer; the server learns their state, never their content |
| Who works on a task | the person | whoever holds its claim: one member on one computer at a time |
| Who lets a module reach the base branch | the person | an Admin's authorization, or the workspace's *automatic merge* |

The Orchestrator acts as the person at this computer: as the member who holds the claim, with that member's rights.
It never gains an Admin's.

## Behaviour that differs today

Each row is a difference a person or the Orchestrator meets. *Closes it* names the task of
`docs/plan/orchestrator-autonomy/` that removes it, or says why it stays. A row leaves the table in the change that
removes the difference.

### Leading work without a person

| Behaviour | Local project | Team project | Why | Closes it |
| --- | --- | --- | --- | --- |
| A module has a branch of its own | no: its tasks merge into the base branch, by default the person's checkout | yes: `module/<slug>`, assigned by an Admin | built for the team server first | OA-17 |
| The module's branch exists in the repository | — | made by its first start when the checkout does not have it (`team_work::start_tip`, 2026-11-14) | a local module has no branch of its own yet | OA-17 |
| A module reaches its base branch | — (no branch of its own) | `merge_run` of a team run merges it into its module's branch and then asks for the module's delivery; the task page has *Merge and deliver* | a local module has no branch of its own to deliver | OA-17, OA-32 |
| The Orchestrator starts tasks in a turn it started | after a success it merged (and, with OA-13, whenever work can start) | only a team task this computer keeps a row of: the start rule reads stored tasks, and a team task that never ran here has none | the server's reading of the task is not used there | OA-29 (the rest) |
| A module subscription leads its tasks' events | yes | yes once the task ran here: its module is where the server put it at the last reading of the watched modules and tasks, else the one its newest run started in. A task the server moved out of every watched module is in no reading and is led by its old module until it runs again; a team task that never ran here raises no event here, and is not among what a module subscription lists | the reading covers what is watched only, and the lists of a watch read this computer's rows | OA-28 (the rest) |
| `/watching` and `get_watches` | answer | refused `team_project_unsupported` | not built | OA-28 |
| How a stopped run goes on (`continue_run`: retry, return, accept, resolve conflicts) | carried out | refused `team_project_unsupported` | not built | OA-29 |
| Continuing an interrupted run (`resume_run`, a message that continues) | carried out | refused | not built | OA-29 |
| Raising a run's limits | carried out | refused `decision_required` (D6): the limits are the Admin's | an open decision of the server's contract | OA-34 |
| A task is Done once its work is merged | set by itself | stays as it was (Backlog, Todo) until its holder or an Admin marks it Done, or its module is delivered | "whether a merged team run makes its task Done is open (D4)" | OA-30 |
| A subscription ends when its tasks are Done | by itself | only once someone marks them Done or the module is delivered: see the row above | follows from it | OA-30 |
| Who may keep working on a task | — | the claim lasts 15 minutes without a heartbeat, and only a run at work renews it: a run that waits for a person, is in review or is merged does not, so the claim goes stale and anyone may take the task over | "human-wait renewal … not covered" (D1, D8) | OA-31 |
| A run whose claim was lost | — | reports nothing more to the server, and keeps executing on this computer | the runner is not told | OA-31 |
| What a kept run end says the chat will do with it (`merging`) | read from the task: Blocked, *Don't merge automatically*, its blockers | read from the run alone: for a task the server holds Blocked, or a run in no module that a Member may not merge, it still says `merges`; the turn then asks the server, is refused (`left_to_person`, `admin_required`) and merges nothing | the follower reads this computer's records, which hold no mark of the server's | OA-35 |
| Where a led target stands (`get_standing`, OA-12) | every task of the target, with its Blocked mark and whether its pipeline is ready | the team tasks that ran on this computer; a Blocked mark the server keeps does not read as parked by the person, and a pipeline's readiness is not read (the server checks it when the work starts) | the standing reads this computer's records, with the server's last reading of the module and of whether a task is closed | OA-28 (the rest), OA-35 |
| The prompt of a turn an event started names the subscription it came through, its order and, when it leads to the end, the leading note (OA-18) | always | only for a team task this computer keeps a row of; one kept only as an identity gets the event's own prompt | the store, which builds the prompt, finds the task's module in a stored row | OA-28 (the rest) |
| Dependencies between tasks (`blocked_by`), tags, story points, *Don't merge automatically* | kept | refused: the server does not keep them | not in the server's contract | OA-35 |
| The ceilings of a subscription that leads to the end (OA-09) | held for every task it leads | found for a team task that ran here; none of the calls they bound work for a team run yet | follows from the rows below | OA-29 |
| Returning a task to draft | `return_to_draft` | refused `team_no_queue` | a team task has no queue on this computer | stays: the server's lifecycle has no draft |
| Forecasts and delivery figures (`get_delivery`) | answer | refused `team_project_unsupported` | not built | OA-36 |
| Tasks of every project in one reading (`list_tasks` without a project) | answer | a linked project is left out | reads the server per project | OA-36 |

### What works the same already

- The Orchestrator's turns: a turn that failed is tried again (OA-01), a used budget or a plan limit holds the turns
  (OA-03), a nearly full session is compacted first (OA-05), and *Let the Orchestrator drive* is its whole consent to
  answer (OA-11). None of them reads the kind of project.
- A team task's run merges on this computer into the branch it started from: its module's branch, or for a task in
  no module the checked-out branch (`b4abf405d` on master, 2026-11-13). The merge queue, the task page, the Review
  page and `merge_run` take it.
- The Pipelines page is one hub for both kinds (`847fe5672` on master).
- The Orchestrator merges a finished run on its own by one rule: a team run into its module's branch, a local run
  into its base branch (OA-29). A merge card the person applies takes a team run the same way.
- Waits and run ends of a team task's runs are kept and start turns, when a subscription leads the task.
- `list_tasks`, `get_task`, `list_modules` and `get_module` read a team project from its server.
- A watch on a team task ends on the status the server reports (`docs/design/orchestrator-watch.md`, *Team tasks*).

### Outside the Orchestrator

Found in the docs while this was written, not read in the code:

| Behaviour | Local project | Team project | Closes it |
| --- | --- | --- | --- |
| A task in no module that lands on the base branch | the person merges it | an Admin merges it, a member only while the team's *Automatic merge* is on | by nature: who lets work reach the base branch (above) |
| A plugin installed on a team server | never sees local projects | works in that server's projects | by design (`docs/architecture/plugins.md`) |

## Keeping the table

- A change that removes a difference removes its row and says so in its commit.
- A change that cannot be built for both kinds adds its row before it merges, with the task that will close it.
- A review checks the change against this page, as it checks `AGENTS.md`.
