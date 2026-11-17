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
| A module has a branch of its own | when the person chose it for the module (`own_branch`): `module/<slug>`, named by Orchester | always: `module/<slug>`, assigned by an Admin | a local module may also merge its tasks into its base branch, as before | stays: the person's choice |
| Who may make a module's acceptance task (`start_acceptance`), and the tasks of its findings | the person, and the conversation that leads the module | an Admin; and, on a team server with this rule (decided 2026-11-17 by the implementing agent, to confirm with the person), the member whose computer holds the module's claim, from the session that took it: `task.create` naming the module (the desktop's `create_task` sends its module with the task) and a task's move into the module are theirs. A member without the module's claim, and every other change of a task by a member, stay refused `not_admin`; a server of 0.11.4 or older refuses the holder too | the module's claim is what lets its holder put tasks in it; an older server keeps the Admins' rule | stays for an older server |
| What decides a module's delivery (`deliver_module`) | the person; the conversation that leads the module, once it is accepted and *Deliver when accepted* is on | an Admin's authorization or the team's *automatic merge*, on the server; the conversation that leads the module asks once its every task is done and work was merged into its branch on this computer, and stays with the module until it is delivered (`to_deliver`: `deliver`, then `person_team_decides` or `person_team_refused`) | by nature: who lets work reach the base branch (above) | stays |
| A module subscription leads its tasks | every task of the module | every task the server puts in the module, read from the server while the module is watched (those that never ran here too), and so the tasks without a module and the whole project while they are watched (2026-11-17). A task the server moved out of every watched module, while neither of those is watched, is in no reading and is led by its old module until it runs again | a reading lists what is watched | OA-28 (the rest) |
| Raising a run's limits | carried out, by a person or by the Orchestrator within its subscription's ceilings (OA-09) | asked of the team server first (`work.raise_limits`), within the ceiling a workspace Admin sets in its policy (so many raises of a run, each limit by at most a share of it; OA-34, 2026-11-17); with no ceiling, the default, refused `decision_required` (D6) as before, and beyond it `raise_beyond_ceiling`. The Admin sets the ceiling through the policy route only: no screen of the app or of the team's web app sets it yet | the limits are the Admin's unless the Admin allows raises; the setting needs a design | OA-34 (the rest: the Admin's screen) |
| A task is Done once its work is merged | set by itself, as Orchester's change | set Done on the server by the holder's desktop after the merge, through the patch of *Mark as done*, so the server records it as the holder's own; a refusal (the claim taken over: `not_holder`) is kept with the run and read in `get_run` (`team.done`), but the task page does not show it yet; a run git found merged outside Orchester asks for no Done | a kind of change of its own for automation is a change of the server's contract (D4); the page's line needs a design | OA-30 (the rest) |
| Who may keep working on a task | — | the claim is renewed every minute while this computer holds the task's work: at work, waiting for a person, paused or interrupted here, succeeded and waiting to be merged (until the Done its merge sends is answered), or paused at a limit; a module's own claim is renewed while its delivery goes on here (requested, waiting for an Admin, merging or reporting). Against a team server of 0.11.4 or older only running work is renewed, as before (the desktop sends the running claims alone when the server refuses the rest). A work waiting for a person as the server sees it (`awaiting_human`) is not renewed (D1); the desktop reports no such state | an older server keeps the old rule | stays for an older server; OA-31 closed it for a current one (2026-11-17) |
| A run whose claim was lost | — | fenced: it reports nothing more and is cancelled here at the next round, its worktree kept; the task page says why. The conversation that led it gets no event of its own: a cancel is no run end it follows, and it learns of it when it next reads the run (`get_run`'s `team.fence`, `claim_fenced`) | a run end for a lost claim needs its words in the chat (the app's run-end card) | OA-31 (the rest) |
| What a kept run end says the chat will do with it (`merging`) | read from the task: Blocked, *Don't merge automatically*, its blockers | read from the task too, its Blocked mark as the server's last reading holds it (2026-11-17); for the run of a task in no module that a Member may not merge (no Admin, the team's *Automatic merge* off) it still says `merges`, and the turn then asks the server, is refused `admin_required` and merges nothing | the reading holds no member's role or the team's merge policy, and the run end's card has no words for that reason yet (a reason of its own in `MergeLeft`, shown on the card: a UI change) | OA-29 (the rest) |
| The prompt of a turn an event started names the subscription it came through, its order and, when it leads to the end, the leading note (OA-18) | by the module the task is in | by the module the task's run started in: a task the server moved since is told by its old module's subscription until it runs again | the store, which builds the prompt, does not hold the server's reading | OA-28 (the rest) |
| Story points | set by a person or by automation (`create_task`, `update_task`), kept with who set them; Orchester estimates them when Settings lets it, and never over a person's | kept on the team server and set like its other details, by an Admin (OA-35, 2026-11-17); the server keeps no say of who set them, and Orchester makes no estimate of a team task | the server keeps a number only | not planned yet |

| Returning a task to draft | `return_to_draft` | refused `team_no_queue` | a team task has no queue on this computer | stays: the server's lifecycle has no draft |
| Cumulative flow (`get_delivery`, Usage › Delivery) | each day's tasks by workflow status, from each task's history of changes | none (`cumulative_flow: {unavailable: team_no_status_history}`, and the card says so) | the server keeps no history of a task's workflow status | not planned yet: needs the team server's contract |
| What the delivery figures count as merged (throughput, cycle time, the forecasts' history) | every merge of the project's tasks, in Orchester or found by git | the merges made on this computer: a teammate's computer's are not known here. Usage › Delivery reads at most 2000 of the team's tasks, and says so when there are more | the server keeps no record of when a task's work was merged | not planned yet: needs the team server's contract |

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
- The chat's tools that name a task (`run_task`, `watch`, `park_task`, `wait_on_task`, the start in a turn Orchester
  started, a task it creates coming back to the conversation that watches) find a team task that never ran here and
  that nothing watches by asking its team server (2026-11-17).
- A message that runs an ended task again (`send_message`) runs a team task again too: the message is kept here for
  the task's next run, which starts through the team server as `run_task` starts it; the ceilings of a subscription
  that leads to the end (OA-09) bound it, `continue_run` and `run_task` for a team run as for a local one
  (2026-11-17).
- Where a led target stands (`get_standing`, OA-12) reads a team task's pipeline as ready when it is one of the team's
  this session read (as a local task's must be published), and the server's Blocked mark whatever watches the task;
  `/watching` and `get_watches` name what waits in a team task's run (2026-11-17).
- `list_tasks`, `get_task`, `list_modules` and `get_module` read a team project from its server.
- Profiles, instructions, skills and command tools are made and changed through the tools, so through the
  Orchestrator chat, in both kinds: a team project's save publishes the team's next version on its server at once (or
  its v1), as the hub's Save does, with the same `person_only` rules (`team_mcp`'s `team_profiles` and
  `team_resources` tests).
- A module's acceptance before its delivery (OA-32, 2026-11-17): a team module's acceptance pipeline (one of the
  team's) and *Deliver when accepted* are kept on this computer and chosen on its page or by `update_module`, by a
  Member too; `start_acceptance` makes the acceptance task on the team server in the module and starts it through the
  team's work start; the conversation that leads the module raises *accept*, then *deliver*, by the local rule.
- `get_delivery` and Usage › Delivery (OA-36, 2026-11-17): built from the team server's tasks and modules and this
  computer's runs, with the two differences in the table above.
- A team task's blocking tasks (`blocked_by`), tags and *Don't merge automatically* are kept on its server and set
  through `create_task` and `update_task` as a local task's (OA-35, 2026-11-17; an Admin's change, as every detail of a
  team task): a task waits for its blocking tasks before `run_task` starts it, and a merge nobody asked for is left to
  the person for the same reasons as a local one.
- A watch on a team task ends on the status the server reports (`docs/design/orchestrator-watch.md`, *Team tasks*),
  and a merged team task is set Done there by the holder's desktop (OA-30), so a subscription ends when its tasks
  are merged, as a local one does.

### Agent testing

Read from the code when `module/agent-testing` took `master` (2026-11-16), not run against a team server. Services,
stacks and test suites start, run and show the same in both kinds of project; what differs is who keeps them.

| Behaviour | Local project | Team project | Why | Closes it |
| --- | --- | --- | --- | --- |
| Services, stacks, test suites and the flaky-test quarantine (Project settings › Services) | kept on this computer | kept on this computer too, by each member for themselves: the team does not share them, though they are a project's configuration | the server's contract keeps none of agent testing's kinds (`docs/cloud-api.md`, `team_resource`) | AT1 (`docs/plan.md`) |
| A Test step in a pipeline | saved and run | saved and run from team server 0.11.4 on (a module led to its delivery against it on 2026-11-16): the step runs the test suites of the computer the run is on, which each member keeps for themselves (the row above). An older server refuses such a version (`422 invalid`, `team_server_refused`), and the save says it as the Test step's: update the team's server, or remove the step. Desktop 0.6.0 does not know the step and cannot use a pipeline that has one | the suites are each computer's until the team shares them | AT1 |

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
