# Orchestrator autonomy — task tracker

What is missing for the Orchestrator to lead tasks and whole modules to the end while nobody is at the computer, and
the tasks that close it. Written 2026-11-13 from a read-only analysis of the code on `master` `2ab1600da`; nothing here
is built yet.

- **Today.** With *Let the Orchestrator drive* on, the chat is woken by events of the tasks a conversation leads and
  acts with the app's tools (`docs/design/orchestrator-toolset.md`, decision 14; `docs/design/orchestrator-watch.md`;
  `docs/manual/orchestrator-chat.md`). It reacts; it does not carry work to the end, and the reaction is lost when
  anything around the turn fails.
- **Goal.** A person subscribes a conversation to a module, says *lead it to the end*, and leaves. They come back to
  every task merged or left to them with what exactly they must do, a list of what the Orchestrator decided on its
  own with its reasons, and nothing lost on the way.
- **Scope.** Local and team projects alike (the person, 2026-11-13: `docs/design/local-and-team-parity.md`). Every
  task here is built for both kinds; what a team project still does differently is in that document's table, and
  block G closes it. The plan was first written for local projects only, which was a mistake of its author.
- **Design.** Visible changes get frames on the desktop design's boards `Orchestrator watch · redesign` `QOPcg` and
  `Orchestrator WIP` `mPw5l` (`crates/desktop-app/design/orchester.pen`) before they are built; tasks of kind
  *design + UI* start with them.
- **Docs.** A task that changes a rule changes it in `docs/design/orchestrator-toolset.md`,
  `docs/design/orchestrator-watch.md` and `docs/manual/orchestrator-chat.md` in the same change.
- **How to take a task:** as in `docs/plan.md` (*How to take a task*).
- **Updating this file:** set a task's status, branch and tokens here and in its own file in the same change.

Status: `todo` · `blocked` (waits for a task) · `needs decision` (waits for the person) · `in progress` · `review` ·
`done` · `canceled`.

## What was found

Each line is what the code does today; the task that changes it is at its end.

**The loop loses events.**

- An event gets one turn. A turn that fails (the CLI crashed, the 10-minute limit, signed out, a plan limit, the
  runner did not take it) ends failed and is never asked for again: `start_wait_turn` gives an entry its `message_id`
  once (`crates/store/src/project_db/orchestrator_waits.rs`), and `send::answer_wait` says "a wait starts one turn at
  most" (`crates/workspace/src/orchestrator/send.rs`). → OA-01, OA-02
- The Orchestrator's turns count toward the daily budget, and a used budget refuses the turn; the event is still
  spent. Nothing wakes the chat when the budget's day turns or when its account's plan limit resets: runs have reset
  timers (`crates/runner/src/execution/coordinator/plan_limit/`), chat turns do not. → OA-03
- One connection and one account, with no second one to go on with. → OA-04
- A conversation is compacted only when a person presses *Compact*. → OA-05
- A stall is raised once per stall, and its turn is told to start, resume and cancel nothing. → OA-06
- Each event is a turn of its own, one at a time per conversation; the watch design says events that wait are taken
  together. → OA-07
- The follower lives in the app's process and the runners do not outlive the window; a sleeping computer stops
  everything. → OA-08

**Nothing carries a module to the end.**

- In a turn Orchester started, `run_task` starts a task only in the turn of a success the chat merged itself, under an
  order, for a task with no run yet (`crates/workspace/src/orchestrator/led_start.rs`: `not_merge_turn`, `not_led`,
  `already_ran`). So a person starts the first wave, a task that failed is never run again, and a start an agent slot
  refused comes back only with the next merged success: when the last working task fails, the module stops for good.
  → OA-12, OA-13, OA-14
- The chat cannot say "this one needs the person" and go on: a task it cannot move simply waits. → OA-15
- A local module has no branch of its own and no delivery: its tasks merge into the base branch, which by default is
  the person's checkout, and a checkout with local changes sends the merge to the Local integration page. → OA-16,
  OA-17
- What limits the chat's own doing is its prompt: `continue_run`'s raises and retries have no ceiling, and the rule
  that its own doing starts no turn (`raised_by_own`) is gone since decision 14. → OA-09

**A person is still needed in the middle.**

- `answer_question`, a `send_message` that answers and `decide_plan` need *Let automation answer approvals* beside the
  drive switch (`crates/workspace/src/app_mcp/tools/questions.rs`); the chat is woken for a question it may not answer.
  → OA-11
- Command requests in the person-only categories (a push, the network, an install, a write outside the worktree,
  credentials, a command not read to the end) wait for the person with no end. → OA-21

**Quality rests on the pipeline alone.**

- A run is merged when every step passed, and a step the chat itself accepted unverified counts. Only a pipeline
  that ends in a Deliver step checks the merged result (the merge queue); a `merge_run` merge is not checked.
  → OA-19, OA-20
- A Verify step that reads the design needs Pencil open with the document saved; nothing tells the person before
  they leave. → OA-19

**A team project is led even less.** (Added 2026-11-13; the table is in `docs/design/local-and-team-parity.md`.)

- A team task's run merges into its module's branch since `b4abf405d` on master (OA-27, done there); the chat still
  can neither merge nor start nor continue a team run in a turn it started. → OA-29
- A module subscription does not lead a team task's events: the task reads as having no module. → OA-28
- A merged team task stays in Backlog until someone marks it Done, so its watch never ends. → OA-30
- A claim goes stale 15 minutes after its run starts to wait or is in review, and a run that lost its claim keeps
  executing. → OA-31
- A team module's branch must be made by hand; its delivery cannot be asked for by the chat. → OA-33, OA-32
- Limits, dependencies between tasks and forecasts are refused for team tasks. → OA-34, OA-35, OA-36

**The person learns little afterwards.**

- The chat's actions are kept in its conversation and counted in the daily report, without reasons; there is no list
  of what it decided on its own to review, no list of what waits for the person with what to do, and no notification
  that leaves the computer. → OA-22 … OA-25

## Person's decisions (2026-11-13)

- **How it is worked.** One session writes the code itself, task after task, in the worktree
  `.tmp/worktrees/claude-orchestrator-autonomy` on `claude/orchestrator-autonomy-plan`: one commit per task, after the
  check and an independent review; nothing is pushed, and `master` is the person's.
- **Scope now:** every task but OA-04 (later) and OA-24 (canceled).
- **Design:** the person draws the frames. Each visible task waits for its frames; what to draw is in
  [frames.md](frames.md). Everything without a visible change is built first.
- **OA-11 · The chat does not depend on *Let automation answer approvals*.** That setting is for outside MCP clients
  only. While it drives, the chat answers questions and messages, decides plans and changes the limits its tools
  change, with no second switch.
- **OA-17, OA-20 · A local module may have its own branch, and the Orchestrator delivers it** after its acceptance
  passed, when the person ticked *Deliver when accepted* for the module.
- **OA-08 · Only keeping the computer awake** while agents work (a setting, Windows and macOS). Closing the window
  still stops the work; OA-19 warns.
- **OA-11, more (asked again the same day):** the drive switch also covers choosing the reviewer and publishing a
  profile that approves by itself. The daily budget is not the chat's in a turn nobody asked for: it changes it only
  when a person's message asked.
- **Block G is this session's**, in this branch, with `master` merged in as it moves.
- **OA-04 · No second connection now.** The chat waits for its account's reset (OA-03).
- **OA-24 · Nothing leaves the computer.** No webhook; OS notifications only.
- **Ceilings of a leading subscription (OA-09):** 3 runs of a task, the first included; 4 returns or retries of the
  chat's own per run; 1 raise of a limit per run by at most 50 %; an optional token budget. Changed in Project
  settings.

## Assumed until the person says otherwise

Not asked yet; the tasks are built on them and each is one constant or one sentence of a prompt to change.

- **Retries (OA-01):** a failed turn Orchester started is tried again after 1, 5 and 15 minutes; after the fourth
  failure the event is left to the person and counts in *Needs you*. A turn the person stopped is not tried again.
- **A used budget holds, it does not fail (OA-03).** No share of the budget is reserved for the Orchestrator.

- **Compaction (OA-05)** runs before a turn Orchester starts once the session used 70 % of its window.
- **A stall is raised again (OA-06)** each further period without progress, three times at most per stall: the first
  turn messages the agent, the second stops it and continues from the last checkpoint, the third parks the task.
- ***Lead to the end* is chosen only by the person (OA-09)** with the watch command (a click; not by the agent's
  `watch` tool, not by synthetic input). *Follow*, today's behaviour, stays the default.
- **Person-only requests stay person-only (OA-21).** A led task whose agent asks for one is parked.
- **Unverified steps (OA-18).** While it leads, the chat accepts a step as it is only when what could not be checked
  cannot be checked at all; a tool that did not answer is tried again and then parked. Every such acceptance is a
  decision to review.

## Tasks

| ID | Task | Kind | Depends on | Status |
| --- | --- | --- | --- | --- |
| **A** | **The loop holds** | | | |
| [OA-01](OA-01-failed-turn-keeps-event.md) | A turn Orchester started that failed leaves its event open | core + store + workspace + app | — | done |
| [OA-02](OA-02-retry-clock.md) | The event's line says what happens to a turn that failed | design + UI | OA-01, frames | built; design verification pending |
| [OA-03](OA-03-budget-and-plan-limit-hold.md) | A used budget and a plan limit hold turns until the reset | core + runner + workspace + UI | OA-01 | done; what is seen built, design verification pending |
| [OA-04](OA-04-second-connection.md) | A second connection for the Orchestrator | design + UI + runner | OA-03 | later (person, 2026-11-13) |
| [OA-05](OA-05-compaction-before-a-turn.md) | The conversation is compacted before a turn that would not fit | workspace + runner | — | done |
| [OA-06](OA-06-stall-again.md) | A run that stays stalled is raised again, and its turn may act | core + workspace | OA-09, OA-15 | done but for the long tool call |
| [OA-07](OA-07-events-together.md) | Events that wait in one conversation are taken in one turn | core + workspace | OA-01 | done |
| [OA-08](OA-08-window-closed.md) | Agents keep working with the window closed and the computer awake | app (Windows and macOS) + UI | frames for its setting | done; its row has no frame, macOS not run |
| **B** | **What the chat may do on its own** | | | |
| [OA-09](OA-09-lead-mode-and-ceilings.md) | Core: a subscription leads to the end, with ceilings | core + store + workspace | — | done; choosing it waits for frames (B) |
| [OA-10](OA-10-choosing-lead.md) | Choosing *Lead to the end*, its ceilings and what is used | design + UI | OA-09, frames | built; design verification pending |
| [OA-11](OA-11-drive-answers.md) | The drive switch is the chat's consent to answer | workspace | — | done |
| **C** | **A module to the end** | | | |
| [OA-12](OA-12-standing.md) | Core: where a led target stands | core + workspace | OA-09 | done |
| [OA-13](OA-13-work-can-start.md) | The event *work can start* | core + workspace | OA-09, OA-12 | done; its line follows its frame, design verification pending |
| [OA-14](OA-14-run-again.md) | A failed task is run again within its ceiling | workspace | OA-09, OA-13 | done |
| [OA-15](OA-15-parking.md) | Parking a task for the person, and a led target that is stuck | core + store + workspace | OA-12 | done; shown on the Overview and the task's Summary (OA-23) |
| [OA-16](OA-16-map-merges.md) | Map: how a run merges into a base branch that is not checked out | research | — | done |
| [OA-17](OA-17-module-branch.md) | A local module's own branch and its delivery | design + core + workspace + UI | OA-16, frames for its UI | built; design verification pending |
| [OA-18](OA-18-leading-instructions.md) | The instructions of a leading turn | core | OA-12 … OA-15 | done; not run with a real model |
| **D** | **Quality without a person** | | | |
| [OA-19](OA-19-before-you-leave.md) | Before you leave: is what the conversation leads ready | design + workspace + UI | OA-09, frames for its UI | built; design verification pending |
| [OA-20](OA-20-acceptance.md) | A module's acceptance before its delivery | core + workspace + UI | OA-17 | built; design verification pending |
| [OA-21](OA-21-person-only-requests.md) | A person-only request in a led run parks its task | workspace | OA-15 | done as instructions |
| **E** | **The record for the person** | | | |
| [OA-22](OA-22-decisions.md) | The chat's own decisions with their reasons | core + store + workspace | — | done; shown on the Overview (OA-23) |
| [OA-23](OA-23-while-you-were-away.md) | *While you were away*: decisions to review and what needs you | design + UI | OA-15, OA-22, frames | built; design verification pending |
| [OA-24](OA-24-outside-notification.md) | A notification that leaves the computer | design + app | — | canceled (person, 2026-11-13) |
| [OA-25](OA-25-daily-report.md) | The daily report names reasons, what is parked and what is held | workspace | OA-03, OA-15, OA-22 | done in the tool's answer |
| **G** | **The same in team projects** | | | |
| [OA-27](OA-27-team-run-merges-into-its-module.md) | A team task's run merges into its module's branch | core + workspace | OA-16 | done on master (`b4abf405d`) |
| [OA-28](OA-28-subscriptions-lead-team-tasks.md) | Subscriptions lead team tasks by their module, and `/watching` answers | workspace | — | done; three edges left (its file) |
| [OA-29](OA-29-chat-acts-on-team-runs.md) | The chat merges, starts and continues team runs as it does local ones | workspace | OA-27, OA-28 | done but a message that runs a team task again |
| [OA-30](OA-30-team-task-done-after-merge.md) | A team task is Done once its work is merged, as a local one is | workspace (and the contract, if the server must agree) | OA-27 | needs decision |
| [OA-31](OA-31-claims-while-work-is-led.md) | A claim holds while its work is led, and a run that lost it stops | contract + workspace + runner | — | needs decision (contract) |
| [OA-32](OA-32-team-module-delivery.md) | The Orchestrator asks for a team module's delivery, as it delivers a local one | workspace | OA-17, OA-20, OA-27 | done (2026-11-17): a team module's acceptance and delivery as a local one's; a Member's computer cannot make the acceptance task (the server's `task.create` is the Admins') |
| [OA-33](OA-33-module-branch-is-made.md) | A module's branch is made when its first task starts, in both kinds | workspace + runner | OA-17 | done |
| [OA-34](OA-34-limits-of-team-runs.md) | A team run's limits are raised as a local run's are | contract + workspace | OA-29 | needs decision (contract) |
| [OA-35](OA-35-team-task-organization.md) | Dependencies between team tasks, and the rest of a task's organization | contract + workspace | — | needs decision (contract) |
| [OA-36](OA-36-team-reads.md) | Forecasts and cross-project readings answer for team projects | workspace | — | `list_tasks` across projects and `get_delivery` done (2026-11-17); no cumulative flow and only this computer's merges for a team (the server's contract); S12 step 4's reads left |
| **F** | **Proof** | | | |
| [OA-26](OA-26-unattended-proof.md) | An unattended module, end to end, with the fake agents | tests | OA-01 … OA-03, OA-06, OA-07, OA-09, OA-12 … OA-15, OA-18, OA-21, OA-22 | a local module of three tasks runs end to end; the full scenario and the team run are left |

## Order

1. **First:** OA-01, OA-05, OA-09, OA-16 (research), OA-22, OA-11; the person draws the frames of `frames.md`.
2. **The loop holds (milestone 1):** OA-02, OA-03, OA-07 after OA-01. This alone makes today's *Follow* stop losing
   events, and ships without the rest.
3. **A flat module to the end (milestone 2):** OA-12, then OA-13 and OA-15, then OA-14, OA-06, OA-21 and OA-18;
   OA-10 and OA-19 beside them. *Flat* means its tasks merge as today, into the base branch.
4. **The record (milestone 3):** OA-23 and OA-25 after OA-15 and OA-22.
5. **The same in team projects (block G):** OA-28 and OA-33 at once (desktop only, no dependency); then OA-29 and
   OA-30 (OA-27 came with master); OA-36 beside them. OA-31, OA-34 and OA-35 change the server's contract: decided with the person,
   changed here first, and the platform's side is theirs. OA-32 after OA-17 and OA-20.
6. **Proof:** OA-26, in a local project and in a team project against the stand-in server, before anyone leaves a
   real module to it.
7. **Then:** OA-17 and OA-20 (the module's own branch, its acceptance and delivery) and OA-08 (awake). The visible
   tasks follow their frames whenever those arrive.

## QA round (2026-11-14, head `5c64909fd`)

In a QA copy (`tools/qa-app.ps1 -Name autonomy -Scenario orchestrator-watch -FakeAgents`), through its MCP server:

- `get_standing`, `get_decisions` and `get_daily_report` answer in the running app; the seeded project's database
  migrated to schema 176.
- A conversation leading the module *Webhooks v2* to the end reads `can_start` with its three draft tasks; after a
  change of the project the follower kept *Work can start in Webhooks v2: KON-10, KON-11, KON-12* in the
  conversation, once. With the three tasks parked it kept *Webhooks v2 is stuck: 3 left for you*, once;
  `get_standing` gave each task as `parked` with what the person is to do, `get_decisions` the parking's decision,
  and the day's report its `led` section. Both lines show in the window.
- Not reached there: the person's `/watch-module` with *lead to the end* typed in the composer (the window tools did
  not open the command menu; the leading subscription and the parkings were written into the QA copy's own
  database), a turn of the Orchestrator (the switch stayed off: its CLI there is a real one), and `park_task` called
  by an agent. These are covered by the automated tests only.
- Found: the follower runs when something of the project changes, so a leading subscription raises its first event
  with the next change, not by a clock. A subscription the person makes in the window is such a change.

In the installed app (`E:\Orchester`, the debug build of this head): the real project opened, its database migrated
from schema 173 to 176, the runner attached, nothing was kept or started, and the log has no error of this work. Its
Orchestrator is not signed in, so no turn could be seen there.

## Second QA round (2026-11-14, head `4882fe039`)

In a QA copy (`tools/qa-app.ps1 -Name autonomy -Scenario orchestrator-watch -FakeAgents`), through its MCP server.
The installed app was not used: another copy of the app was at work on the person's own project, whose database this
head would have migrated from schema 176 to 178.

- **A module's own branch.** `update_module` with `own_branch: true` gave *Webhooks v2* `module/webhooks-v2`;
  `get_module` showed it with `waits_for`. `run_task` of one of its tasks made the branch in the repository at
  `main`'s tip, and the run recorded it as its base (`source: module`), the checkout staying on `main`.
- **Delivery.** `deliver_module` answered `nothing_to_deliver` before any work, `runs_at_work` while that run had not
  ended, and, with a commit on the module's branch, merged it into `main` with a merge commit that reached the
  checkout; `get_module` showed the delivery, and a second call answered `nothing_to_deliver`. `start_acceptance`
  answered `no_acceptance_pipeline`.
- **Before you leave.** `get_standing` gave, for the conversation leading the module to the end, `ready: false` with
  `drive_off`, `no_deliver_step` (its three tasks), `may_sleep` and `stops_with_window`, and what is not checked.
- **Keep the computer awake.** The row shows in Settings › General under *Work*; its switch turned on through the
  window and the setting was stored.
- **Not reached there:** a run that works to its end (the QA copy's fake agent program did not start, so the run
  stopped at its first step: nothing held the computer awake, no success was merged into the module's branch by a
  run, and no acceptance ran); a turn of the Orchestrator, so neither events taken together (OA-07) nor the turns of
  *accept* and *deliver* were seen live; a team project. These are covered by the automated tests only.

## Edge cases and what settles them

| Case | Today | Settled by |
| --- | --- | --- |
| The Orchestrator's turn fails or times out | the event is spent | OA-01: tried again 3 times, then left to the person |
| A turn timed out after it merged or answered | — | OA-01: the next try is told how the last ended and reads the state first; the tools refuse what was done |
| The daily budget is used at 23:00 | every event until the reset is spent | OA-03: events wait, turns start at the reset |
| The Orchestrator's account is at its plan limit | turns fail | OA-03 holds until the reset; OA-04 goes on elsewhere |
| A run and the Orchestrator share the account at its limit | the run waits for the reset, the chat fails | OA-03: both wait for the same reset |
| 300 event turns in one conversation | the CLI's own compaction, or a failure | OA-05 |
| A run stays stalled for hours | one turn, which may only write | OA-06: raised again, acts, then parks |
| A long build inside one tool call reads as no progress | a stall turn | OA-06: a tool call under way is not a stall before its own timeout |
| Twelve events arrive while one turn thinks | twelve turns, one after another | OA-07 |
| The window is closed, or the computer sleeps | everything stops | OA-08; until then OA-19 warns |
| Nobody started the module's first tasks | nothing starts | OA-13 |
| Every agent slot is taken | told, never tried again | OA-13: raised again when a slot frees |
| A task failed and others wait for it | the module stops | OA-14 runs it again; past its ceiling OA-15 parks it and the stuck event tells the person |
| The chat returns a run, it fails, it returns it again | until the daily budget is used | OA-09: `ceiling_reached` |
| The chat raises a limit again and again | no ceiling | OA-09 |
| A question that the brief does not answer | told to the person, the run waits | OA-18: decided by the stated precedence and recorded (OA-22), or parked when it is irreversible or outward |
| An agent asks to push or to use the network | waits for the person | OA-21: parked, the rest goes on |
| A plan waits for a decision | the person's, or a second switch | OA-11 |
| Verify could not reach Pencil | accepted as unreachable, or waits | OA-18: tried again, then parked |
| The person's checkout has local changes | the merge goes to Local integration | OA-17: a module's own branch; without it OA-19 warns and OA-15 parks |
| Two running tasks change the same file | a conflict after the fact, resolved by an agent | OA-18: the chat orders them with `blocked_by` when it plans; base-moved notices and `resolve_conflicts` stay the net |
| A merge broke the base branch | nobody notices | OA-19 requires a Deliver step for a leading subscription or warns; OA-20 for a module's delivery |
| Two conversations act on one run | only `run_task` checks who leads | OA-09: the ceilings count per run, whoever acted; no further change planned |
| The person takes a task over while the chat leads it | both act | OA-15: a person's action on a parked or led run settles the chat's event for it |
| The person turns the drive switch off mid-way | no new turn starts | OA-01, OA-13: no retry and no new event while it is off; what waited starts nothing when it is on again unless it still stands |
| The low-disk pause | runs pause and go on by themselves; no event | OA-12: the standing says *held: low disk*; the stuck event names it |
| A led task has no pipeline, or its pipeline is not ready | the start is refused | OA-19 before leaving; OA-15 parks it |

## Not here

- The server that will run projects without a desktop.
- A list of person-only command categories the chat may allow.
- File ownership between running tasks as a mechanism of the app; the chat orders tasks instead (OA-18).
- Reverting a merge automatically: the Deliver step's check of the merged result comes before the merge instead.
- The watch design's second wave other than the budget of a subscription: pipelines, profiles and runners as
  targets, watch-only subscriptions, digests.
- Planning a module from its design (the coverage audit of `.claude/skills/work-outside-orchester`): the chat leads
  the tasks a module has.

## Design verification (2026-11-14)

Two independent verifiers compared the frames of `Orchestrator autonomy · frames to approve` with QA copies of head
`a880ac581`, at 1440 × 900: no blocking mismatch. What they found and what was changed after it is in the task
files (OA-02, OA-03, OA-10, OA-15, OA-17, OA-23). The fixes were not verified again. Not reached by them: an
answered turn of the Orchestrator (the `orchestrator-*` scenarios are static: their sign-ins run nothing), the
lines of a stuck target, an acceptance and a delivery, a module that is accepted, *Ready to leave*, and the light
theme.
