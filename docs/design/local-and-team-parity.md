# Local and team projects behave the same

Status: decided by the person on 2026-11-13. The table below is what the code did on that day (`master` `791e494c5`
with the Orchestrator autonomy work of `docs/plan/orchestrator-autonomy/`), read from the code and not run against a
team server.

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
| A run's work reaches its module's branch | `merge_run`, the task page and the merge queue merge it | no path merges a run with team inputs anywhere: `merge_in_as` refuses it (`localIntegration.teamRun`), though the contract says "a team task's run merges into its module's branch" (D3) | the merge was left to "delivery", which carries the module's branch without the run's work | OA-27 |
| A module has a branch of its own | no: its tasks merge into the base branch, by default the person's checkout | yes: `module/<slug>`, assigned by an Admin | built for the team server first | OA-17 |
| The module's branch exists in the repository | — | nothing creates it; a start is refused `branch_not_found` until someone makes it by hand | the docs say the runner makes it; no code does | OA-33 |
| A module reaches its base branch | — (no branch of its own) | `merge_run` of a team run asks for the module's delivery; the chat cannot call it in any turn | the chat's merge goes through the local merge, which refuses team runs | OA-17, OA-32 |
| The Orchestrator merges a finished run on its own | yes, by its rule | never: every team run is left to the person (`team_delivery`) | follows from the first row | OA-27, OA-29 |
| The Orchestrator starts tasks in a turn it started | after a success it merged (and, with OA-13, whenever work can start) | never: the start rule needs a merge the chat cannot make, and reads stored tasks only | follows from the first row | OA-29 |
| A module subscription leads its tasks' events | yes | no: a team task reads as having no module, so its events go to *Tasks without a module* or to nobody | the module is in the run's team inputs and is not read | OA-28 |
| `/watching` and `get_watches` | answer | refused `team_project_unsupported` | not built | OA-28 |
| How a stopped run goes on (`continue_run`: retry, return, accept, resolve conflicts) | carried out | refused `team_project_unsupported` | not built | OA-29 |
| Continuing an interrupted run (`resume_run`, a message that continues) | carried out | refused | not built | OA-29 |
| Raising a run's limits | carried out | refused `decision_required` (D6): the limits are the Admin's | an open decision of the server's contract | OA-34 |
| A task is Done once its work is merged | set by itself | stays as it was (Backlog, Todo) until its holder or an Admin marks it Done, or its module is delivered | "whether a merged team run makes its task Done is open (D4)" | OA-30 |
| A subscription ends when its tasks are Done | by itself | only once someone marks them Done or the module is delivered: see the row above | follows from it | OA-30 |
| Who may keep working on a task | — | the claim lasts 15 minutes without a heartbeat, and only a run at work renews it: a run that waits for a person, is in review or is merged does not, so the claim goes stale and anyone may take the task over | "human-wait renewal … not covered" (D1, D8) | OA-31 |
| A run whose claim was lost | — | reports nothing more to the server, and keeps executing on this computer | the runner is not told | OA-31 |
| Dependencies between tasks (`blocked_by`), tags, story points, *Don't merge automatically* | kept | refused: the server does not keep them | not in the server's contract | OA-35 |
| The ceilings of a subscription that leads to the end (OA-09) | held for every task it leads | not found for a team task led through its module (the task reads as having no module), so nothing bounds the chat there; and none of the calls they bound work for a team run yet | follows from the module row above | OA-28, OA-29 |
| Returning a task to draft | `return_to_draft` | refused `team_no_queue` | a team task has no queue on this computer | stays: the server's lifecycle has no draft |
| Forecasts and delivery figures (`get_delivery`) | answer | refused `team_project_unsupported` | not built | OA-36 |
| Tasks of every project in one reading (`list_tasks` without a project) | answer | a linked project is left out | reads the server per project | OA-36 |

### What works the same already

- The Orchestrator's turns: a turn that failed is tried again (OA-01), a used budget or a plan limit holds the turns
  (OA-03), a nearly full session is compacted first (OA-05), and *Let the Orchestrator drive* is its whole consent to
  answer (OA-11). None of them reads the kind of project.
- Waits and run ends of a team task's runs are kept and start turns, when a subscription leads the task.
- `list_tasks`, `get_task`, `list_modules` and `get_module` read a team project from its server.
- A watch on a team task ends on the status the server reports (`docs/design/orchestrator-watch.md`, *Team tasks*).

### Outside the Orchestrator

Found in the docs while this was written, not read in the code:

| Behaviour | Local project | Team project | Closes it |
| --- | --- | --- | --- |
| The Pipelines page | the hub (`docs/design/pipelines-v2.md`) | the page from before it, with one *Save* | no task yet |
| A plugin installed on a team server | never sees local projects | works in that server's projects | by design (`docs/architecture/plugins.md`) |

## Keeping the table

- A change that removes a difference removes its row and says so in its commit.
- A change that cannot be built for both kinds adds its row before it merges, with the task that will close it.
- A review checks the change against this page, as it checks `AGENTS.md`.
