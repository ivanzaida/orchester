# The Orchestrator's toolset

Status: decided by the person on 2026-10-15; decision 5 and the waits changed by the person on 2026-10-17 (ORC-42);
continuing a stopped run and the ends of runs added to decision 5 on 2026-10-19 (proposed, conservative; see there);
continuing interrupted runs behind *Let the Orchestrator continue interrupted runs* decided by the person on
2026-10-20 (ORC-53); merging finished runs on its own behind *Let the Orchestrator merge finished runs on its own*
decided by the person on 2026-10-21 (ORC-69).
This is the contract the parallel work streams share. Implementation notes and file ownership are at the end.

The Orchestrator chat (ORC-17) gives its agent a part of Orchester's domain MCP tools (`crates/workspace/src/
orchestrator/tools.rs`, `TOOLS`). Every call goes through the same tool an MCP client of the app calls, so it is
refused or carried out exactly as for that client. This document widens that part so the Orchestrator can see
everything a person can see in Orchester, change what is safe to change, propose the rest, and answer with links the
person can click.

## Decisions

1. **Reading is open.** The Orchestrator reads everything in every project the app knows: tasks, runs, changes,
   approvals, questions, plans, tasks' conversations, pipelines, agent profiles, instructions, skills, tools, secrets
   (names and use only), connections, accounts, models, usage and pull requests. Cross-project reads are allowed
   ("what is running everywhere"); changes stay in the chat's own project. A step that waits for a message always
   shows what its agent wrote last (`get_run`, `list_questions`), so a wait is never unreadable.
2. **Drafts are free.** It saves drafts of tasks, pipelines, agent profiles, instructions, skills and tools without
   asking. A draft reaches no agent's prompt.
3. **Publishing needs the person, or the person's switch.** Publishing a pipeline, profile, instruction, skill or
   tool is proposed in the chat as a card the person applies with a click. A new person-only switch in Settings ›
   What automation may do, *Let the Orchestrator publish drafts*, lets the chat publish directly. Synthetic input
   cannot flip it (the same rule as *Let automation change agent permissions*). Widening what agents may do stays
   behind *Let automation change agent permissions* in every case. This subsumes ORC-30 for the chat.
4. **Starting runs: both.** By default a start is a proposal card the person starts (as today). When auto mode is on
   for the project, the chat starts the run directly, through the same scheduling checks; the start is kept in the
   conversation as an action. Its neighbours follow the same pattern behind their own switches: answering agents
   and merging runs (decision 5); canceling a run is always a proposal.
5. **Answering agents and merging: the person's click, or the person's switch** (changed 2026-10-17, ORC-42).
   - The chat may answer an agent's question (`answer_question`) and send a run's agents a message (`send_message`).
     Each is a proposal card the person applies by default; with the person-only switch *Let the Orchestrator answer
     agents* (Settings › What automation may do) the chat sends it directly. Every answer is recorded as automation's
     (`decided_by` automation) and the task's Conversation shows it as sent by the Orchestrator. A message that would
     decide the plan a run waits on is refused: plan decisions stay the person's.
   - The chat follows the agents of its project: when an agent asks a question, waits for a message or its plan waits
     for a decision, a conversation gets an entry linking to its card: the one that last dealt with that task or its
     run (decided by the person on 2026-10-17): the conversation whose newest entry about them is the newest, where an
     entry about them is an action the Orchestrator took on the task or run (done, proposed, refused or dismissed) or
     an earlier wait of the task. When no conversation dealt with it, the project's latest conversation gets it, or a
     new one titled after the wait while the project has none; waits of different tasks may so land in different
     conversations. With *Let the Orchestrator answer agents* on, the wait also starts one Orchestrator turn by itself
     in its conversation: at most one per wait, never while another turn of that conversation runs (it waits for that
     turn to end; a turn of another conversation does not hold it), and none for a wait the Orchestrator's own answer
     raised. In that turn the Orchestrator answers only what follows plainly from the task's brief and criteria, the
     project's docs and AGENTS.md, or what the person already said in the chat; otherwise it tells the person what is
     asked, with options and its recommendation.
   - The chat may merge a run (`merge_run`): a proposal card by default; with the person-only switch *Let the
     Orchestrator merge runs* it merges at once, only a run the person asked it to merge, or a finished run it merges
     on its own (below). The merge keeps all of
     `merge_run`'s rules: only a task's latest run that succeeded, only when the main checkout needs no local
     integration, else refused (`needs_decision`, `merge_failed`); conflict choices and approving the merged result
     stay the person's, on the Local integration page. The merge is kept in the conversation as an action. A
     `merge_failed` of the chat's merge names the next step: for a merge that stopped at conflicting files, proposing
     `continue_run` `resolve_conflicts` (below), naming the files; otherwise the Local integration page.
   - The chat merges a finished run on its own behind the person-only switch *Let the Orchestrator merge finished
     runs on its own* (Settings › What automation may do; decided by the person on 2026-10-21, ORC-69). It needs *Let
     the Orchestrator merge runs* and *answer agents* on too, and the switch's row says so. With all three on, the turn
     a task's run success starts by itself (below) merges the run with `merge_run`, directly, only when all of these
     hold (one rule, `orchester_workspace::orchestrator::auto_merge`): it is the task's latest run and succeeded, and
     every step of its pipeline passed — a step a person accepted as it was counts as passed, one automation accepted
     does not; the task is not marked Blocked and no unfinished task blocks it (ORC-58); new starts are not held for
     its reviewed group or the project (`hold_queue`); the task does not say *Don't merge automatically*, a task
     organization flag set in the task form or with `create_task`/`update_task` (`no_automatic_merge`); and the run is
     not the chat's own doing in a loop (`raised_by_own`: such an end starts no turn). `merge_run`'s own rules hold on
     top: a main checkout that needs local integration is refused (`needs_decision`, `scoped_integration_required`).
     The rule is read when the end is kept (the entry says whether the turn merges the run or why it is left to the
     person, `merging`) and again when the turn calls `merge_run`: a run it leaves to the person by then is refused
     (`left_to_person`), naming why. One automatic merge attempt per run end: the turn merges only the run whose
     success started it, once, and a second `merge_run` in that turn is only proposed. A `merge_failed` at conflicts
     leads the turn to propose `continue_run` `resolve_conflicts` (a card, as for any merge of the chat's); after the
     agent resolved them and the run succeeded again, that is a new end, merged again under the same rule. Any other
     refusal is told to the person with its reason and the merge left to them. Runs that end close together are merged
     one at a time in the order they ended: a turn that merges its run does not start while one that ended before, in
     any conversation, still waits for its turn or thinks (ORC-54's base-moved notices may then go to other runs). Each
     such merge is an action in the conversation, *merged by the Orchestrator on its own* with the revision it moved
     the base branch to and the task's link (`ChatAction::on_its_own`); the task's Conversation shows the
     Orchestrator's row under the run's end with the way back to the chat's conversation (ORC-45's attribution), and
     `get_conversation` names the Orchestrator as the merge's author. OS notifications treat it as a merge the chat
     carried out itself (no notification of its own, ORC-56), as will the daily report (ORC-47) once it exists.
   - These switches are off until a person turns them on; synthetic input cannot flip them (the same rule as *Let
     automation change agent permissions* and *Let the Orchestrator publish drafts*).
   - The chat may propose how a run that waits for a person's decision goes on (`continue_run`): only `allow_retry`
     (one more retry of the step that used up its retries or broke again, or the step that waits unverified or
     unchanged run again), `return_to` (an earlier agent step, with a note for its agent) and `resolve_conflicts`
     (below). It is always a proposal card the person applies or dismisses, showing the run, the step it stopped at
     and why, the choice and the note.
     Applying is the person's own decision, recorded as theirs, exactly as the task's Summary decides it; it is
     refused when the run went on since it was proposed (a newer attempt). No switch lets the chat continue such a
     run itself: none of the person's switches was given for it (*answer agents* is about what agents asked, *merge
     runs* about merging, *continue interrupted runs* about runs a crash, the runner or *Quit now* interrupted), and a
     retry spends more of the budget and overrides the retries the pipeline allows, which the
     pipeline left to a person. `raise_limits`, `raise_step_budget`, `resume` and `accept` stay the person's and are
     refused (`person_only`). Where `continue_run` returns a run that ended (succeeded or failed) to an earlier agent
     step (ORC-44), the chat may propose that `return_to` too, under the same rules; a turn a failure started may
     propose it.
   - The chat may propose sending a run whose last merge into its base branch stopped at conflicts back to an agent
     (`continue_run` `resolve_conflicts`, ORC-52): exactly what the task page's *Ask an agent to resolve* does, through
     the same code (`tasks::return_work::resolve_command`): the work goes back to a step whose agent may write in the
     worktree (the step the call names, else the one that implemented what conflicts, else the last writing step),
     with the conflict's note (`conflict_note`) and the chat's optional note after it, and the conflicting files as
     data beside the note. Always a proposal card naming the run, the step, the base branch, the conflicting files and
     the note; applied, it is the person's decision, recorded as theirs, and the failed merge is forgotten as after the
     page's button. Refused when the run's last merge in the app did not stop at conflicts (`merge_not_conflicted`),
     for a step that may not write (`step_cannot_write`) or a run with none (`no_writing_step`), while git works on the
     run (`integration_running`), and as a return is refused (`run_merged`, `worktree_removed`, `worktree_in_merge`,
     `run_not_ended`). A proposal is refused (`changed_since_proposed`) once the run went on, or its merge no longer
     stands at the conflicts it was proposed for (resolved, merged or returned from the task page).
   - The chat resumes a run a pause stopped (`resume_run`). It continues a run interrupted for a reason that is
     neither a person's choice nor a limit — a crash, the runner going away, or *Quit now* — only behind the
     person-only switch *Let the Orchestrator continue interrupted runs* (Settings › What automation may do; off until
     a person turns it on, synthetic input cannot flip it; decided by the person on 2026-10-20, ORC-53). With it on,
     `resume_run` continues such a run from its last checkpoint through the same path as the task page's *Continue*
     (`runners.continue_run_in`), and the continuation is kept in the conversation as the chat's own action. With it
     off, the call is refused (`person_only`) as before; it never becomes a proposal, since the person chose a
     switch. Whatever the switch says, the person's are, and refused (`person_only`): a run stopped at a limit (its
     token budget or time limit, the daily or a group's budget, a step's budget, an account's plan limit), a run the
     person canceled, a run the runner could not record or carry on, and an interrupted run whose worktree is gone. A
     proposed resume the person applies is checked again: a run that stopped at a limit or was interrupted since is
     refused, not resumed. The start-up recovery (`docs/architecture/storage.md`) keeps offering the interrupted runs;
     the runner continues a run once (one continuation at a time, only while it is interrupted), so the chat's
     continuation and the recovery's *Continue all* never both continue it: the later one is told the run is not
     interrupted any more.
   - A message (`send_message`) reaches an agent that waits for an answer or works, or waits for one, as the task's
     composer sends it, and goes directly under *Let the Orchestrator answer agents*. A message to a run none of
     whose agents waits or works moves the run: it follows a run ready for review up, continues a failed run or one
     that waits for a decision, resumes a paused one, continues an interrupted one, waits for a raised limit, or runs
     an ended task again. Moving a run is the person's: such a message is always a proposal card the person applies,
     naming what sending it does, whatever the switch says. A message keeps what sending it did when it was checked
     (the composer's action, `send_message`'s `action`); sent directly or applied, it goes only as that, and is refused
     (`changed_since_proposed`) once the run's state would make it do something else.
   - The chat follows the ends of its project's runs as it follows the agents' waits: when a task's latest run
     succeeds (ready to merge), fails, stops for a decision (a step used up its retries, broke again, or waits
     unverified or unchanged), stops at a limit (its token budget or time limit, the daily or a group's budget, a
     step's budget, an account's plan limit) or is interrupted, the conversation chosen as for a wait gets one entry
     for it, linking to the task's Summary. Each end is kept once, by the run and the event that put it there: a run
     that stops again after it went on is a new end. The daily budget and a group's budget stop many runs at once: all
     the runs one of them stopped on a day are one entry, naming the first and counting the others as they stood when
     it was kept; a run the same limit stops later that day adds no entry and no turn (it is seen on the Summary and
     in the task list). Not followed: a pause or a cancel, *Quit now* and a stop of a waiting run the person asked
     for, an interruption the runner continues by itself when it comes back, a run that succeeded and was merged (in
     Orchester, or outside it as git tells) or has nothing to merge, and ends from before the project followed them
     (a project starts following when its database gains them, so its history does not flood the chat). Git is asked
     about runs that succeeded once per follow, in one reading of the branches, outside the follower's lock; a run it
     found merged or with nothing to merge is not asked about again.
   - With *Let the Orchestrator answer agents* on, an end starts one Orchestrator turn by itself, under the same rules
     as a wait's (at most one per end, never while another turn of its conversation thinks, only while the run still
     stands so). For a run that succeeded the turn tells the person it is ready to merge; if the person asked earlier
     in that conversation for that run's merge, it calls `merge_run`, which only proposes it. For a run that waits for
     a decision the turn reads the run, tells the person what happened and the choices, and may propose a retry or a
     return (`continue_run`). For a failure, a limit or an interruption the turn reads the run and tells the person
     what happened and the choices they have on the task's Summary, which are theirs. Such a turn starts, resumes,
     cancels and merges nothing itself, with two exceptions. ORC-53: with *Let the Orchestrator continue interrupted
     runs* on too, the turn of an interrupted run's end the chat may continue (a crash; the runner going away and
     *Quit now* are no followed ends) continues that run once with `resume_run`, from its last checkpoint, and tells
     the person. Only the run whose end started the turn, and never in a loop: a run interrupted again within 10
     minutes of the interruption before is left to the person, its turn told why and `resume_run` refused
     (`interrupted_again`); a run the chat continued that is interrupted again starts no turn (`raised_by_own`).
     ORC-69: with *Let the Orchestrator merge finished runs on its own* and *merge runs* on too, the turn of a run's
     success merges that run once with `merge_run` when the rule above lets it, else tells the person why it is left
     to them; such turns start one at a time in the order the runs ended. Each turn's wait keeps what it may do
     (`continuing`, `merging`), and the tool checks it again when it is called.
   - In a turn a wait or an end started by itself nobody asked for anything, so `merge_run` (but the one merge of the
     run whose success started the turn, above), `run_task`,
     `pause_run`, `resume_run` (of a run a pause stopped, or an interrupted run but the one above), `release_queue` and
     every publication only propose, or for an interrupted run are refused (`person_only`),
     whatever *Let the Orchestrator merge runs*, *publish drafts* and auto mode say; the person applies the card. This
     holds by the tools (the turn's tools know the turn started by itself), not by the prompt. A release there is
     refused before it is proposed when nothing is held (`nothing_held`), only a person's hold is (`person_only`) or
     its task is in no reviewed group (`not_in_group`); applied, it is refused the same way when that changed since,
     also when the hold went between the check and the release. In any turn, a `hold_queue` of what is held already
     (`already_held`) or a `release_queue` of what is not held (`nothing_held`) is refused, so its card shows no change
     that did not happen. Answering an agent's
     question and a message to an agent that waits or works stay direct under *answer agents*. Not covered:
     `create_task` and `update_task` still save a task's details directly in such a turn, budgets and time limits of
     its future runs among them; they reach no run until a start, which is proposed.
   - No turn starts for a wait or an end the chat's own doing led to (`raised_by_own`), so the chat never takes up its
     own doing in a loop: the newest entry about the run, before a wait of the run, is its answer, message,
     continuation or resume of the run (applied or done itself, the resume matched by run); for an end, also its own
     start of the run, or the run its message that ran the task again started (kept on the action when the person
     applied it, so a later run of the task a person starts gets its turn). A run that succeeded is the chat's own only
     when its message moved the run (a follow-up, a continuation, a resume) or its message started it; an answer or a
     continuation
     does not count there, since a success turn only tells the person and proposes. Each wait or end starts one turn
     at most.
   - **Never from the chat:** answering approvals, deciding plans, widening permissions, secret values and deleting
     work (worktrees, drafts, tasks).
6. **Links everywhere.** Every object a tool returns carries an app `link`. The chat renders links and a click opens
   the place in the app.
7. **The daily report (ORC-47).** At the time the person sets in Settings › What automation may do (off until they
   turn it on), each open project's latest conversation (or one it opens) gets one *Daily report* entry per local day
   that had activity (a merge, a run that ended or stopped, an action of the Orchestrator, anything its conversations
   kept, tokens or agent time); a report whose time passed while the app was closed comes at the next start, labelled
   with its day, for the days since the report was turned on, at most 7 back. It is built only from what the project's
   database holds (`orchester_workspace::orchestrator::report`): what the day did, and what stands (runs, waits, the
   queue) as of when it is built. With *Let the Orchestrator answer agents* on, the report starts one turn by itself, as
   a wait does, which adds a summary and recommendations and changes nothing: its tools read from the conversation
   that a report's turn is the one under way (`orchester_workspace::orchestrator::guard`), so every change a card
   carries (a merge, cancel, pause, resume, release, continuation, publication, answer or message) is only proposed,
   whatever the person's switches, and any other change (a task, a draft, a copy of a pipeline, a hold of new starts) is
   refused as `report_turn`; a start is proposed as in any turn nobody asked for. `get_daily_report` reads the
   delivered report of a day, or builds it.

## Tools

`existing` tools are already in the app's MCP server; `new` ones are built by the work streams below. All reads take
an optional `project` (id or name); without it, list tools answer for every project with each item naming its
project.

| Area | Read | Change (draft / free) | Proposed (person clicks, or switch) |
|---|---|---|---|
| Projects | `list_projects`, `get_project` (existing) | — | — |
| Tasks | `list_tasks` (existing; with each task's `blocked_reason` and `held_by_blocking_tasks`, ORC-58), `get_task` (existing; also `no_automatic_merge`, ORC-69); `get_conversation` (new, ORC-42: a task's conversation as its Conversation tab shows it, paged; a run's end merged by the chat names the Orchestrator as its author, with `on_its_own`) | `create_task`, `update_task` (existing; ORC-58: also the manual Blocked's `blocked_reason` — `null` clears it —, the whole list of blocking tasks `blocked_by` by key or id with the form's gate, and `tags` by name, a new name created as the tag picker does; checked as the task form checks them, refused with its reasons (`invalid_organization` for a blank reason or a cycle, `task_not_found` for a task not of the project), recorded in the task's history as automation's. Free organization changes, like the title or priority: a manual Blocked is informational and widens nothing, but the chat's `run_task` refuses a task marked Blocked as `task_blocked`, naming the reason; the person can still start it in the app. ORC-69: also *Don't merge automatically*, `no_automatic_merge`, which keeps the chat from merging the task's runs on its own) | — |
| Runs | `get_run`, `get_changes` (existing); `list_runs` (new: a task's or project's runs, newest first, with phase and links) | `pause_run`, `resume_run` (existing; a run a pause stopped, and with *Let the Orchestrator continue interrupted runs* a run a crash, the runner going away or *Quit now* interrupted, from its last checkpoint; never a run at a limit, canceled, failed by the runner or without its worktree; both, and `release_queue`, only proposed in a turn Orchester started, but for the interrupted run whose end started the turn, continued once under both that switch and *answer agents*) | `run_task` (direct in auto mode, never in a turn Orchester started), `cancel_run`, `merge_run` (direct with *Let the Orchestrator merge runs*, never in a turn Orchester started but for the run whose success started it: merged once with *Let the Orchestrator merge finished runs on its own* when the chat's rule lets it, else refused `left_to_person`, ORC-69; its `merge_failed` names the next step), `continue_run` (`allow_retry`, `return_to` and `resolve_conflicts` only, always proposed) |
| Waits | `list_approvals` (existing); `list_questions` (with the steps waiting for a message and what their agents wrote last), `get_plan` (new, read only) | — | `answer_question`, `send_message` (direct with *Let the Orchestrator answer agents*; a message that moves a run is always proposed) |
| Queue | — | `hold_queue`, `release_queue` (existing) | — |
| Pipelines | `list_pipelines`, `get_pipeline` (existing) | `save_pipeline_draft`, `create_pipeline`, `duplicate_pipeline` (existing) | `publish_pipeline` |
| Agents | `list_profiles`, `get_profile` (existing) | `save_profile_draft` (existing) | `publish_profile` |
| Instructions | `list_instructions` (existing); `get_instruction` (new: text, versions, where it applies) | `save_instruction_draft` (existing) | `publish_instruction` |
| Skills | `list_skills`, `get_skill` (new) | `save_skill_draft` (new) | `publish_skill` (new) |
| Tools | `list_tools`, `get_tool` (existing) | `save_tool_draft` (existing) | `publish_tool` |
| Secrets | `list_secrets` (new: name, scope, which profiles/tools/connections use it; never a value) | — | — |
| Connections | `list_connections`, `list_accounts`, `list_models` (existing) | — | — |
| Usage | `get_usage` (new, with filters: from, to, project, task, run, pipeline, step, agent profile, model, account; grouped by day, task, pipeline, step, agent, model or account) | — | — |
| Pull requests | `list_pull_requests`, `get_pull_request` (new: state, checks, review state, its task and run) | — | — |
| Daily report | `get_daily_report` (ORC-47: a project's report of one local day, the one delivered to its conversation for that day, else read now: merged tasks, runs ready, stopped and at work, what waits for the person, the Orchestrator's own actions, direct or applied, the day's tokens and agent time by task and account against the daily budget, the queue; every item with its `link`) | — | — |

Deciding approvals (`decide_approval`) and plans (`decide_plan`, and a `send_message` that would request a plan's
changes), `continue_run`'s raising of limits and budgets, resuming after the daily budget and accepting a step,
widening what agents may do, secret values, project setup, connections' changes, MCP server changes,
environment, worktree removal and `return_to_draft` stay out of the chat's tools. Answering agents' questions and
sending a run's agents messages are the chat's since ORC-42, behind *Let the Orchestrator answer agents* (decision 5).

## Links

- A `link` field is an app route path, the same strings `crates/desktop-app/src/routes` parses: for example
  `/project/<project-id>/tasks/ORC-24`, `/project/<project-id>/tasks/ORC-24?tab=conversation`,
  `/project/<project-id>/usage?from=2026-10-01&to=2026-10-15&group=task&agent=<profile-id>`,
  `/project/<project-id>/pull-requests/<number>`.
- One builder in `crates/workspace` (`orchester_workspace::links`) makes every link, and the app's router has a test
  that parses every kind the builder makes, so the two never drift.
- External places (a pull request on GitHub) carry `url` beside `link`. The chat shows an external link with its host
  and opens it in the default browser; it never opens an external link on its own.
- In the chat's markdown the agent writes `[ORC-24](orchester:/project/<id>/tasks/ORC-24)`. The chat renders an
  `orchester:` link as an in-app link and a click navigates the active tab. Any other scheme is shown as text with
  its address, and only `https:` opens the browser, on click.
- Pages that do not exist yet (Usage ORC-15, Pull requests ORC-13) get their routes with their pages; until then the
  tools still return the link and the router answers with the "not built yet" placeholder.

## Work streams (in parallel)

| Stream | Owns | Depends on |
|---|---|---|
| A. Chat MCP connection fix | `crates/desktop-app/src/mcp/chat.rs`, the shared protocol versions | — (in progress) |
| B. Reads and links | `crates/workspace/src/app_mcp/tools/` new read tools except usage and PRs; `orchester_workspace::links`; `link` fields on existing results; cross-project reads; router parity test | — |
| C. Chat rules | `crates/workspace/src/orchestrator/` (`TOOLS`, proposals for publish/cancel/merge, direct start in auto mode, cross-project read scope), the *Let the Orchestrator publish drafts* switch (store, Settings › What automation may do, person-only), proposal cards in the chat | B's tool names (this table), design frames |
| D. Links in the chat | the chat timeline's markdown links (`orchester:` and `https:`), navigation, external link presentation | B's link format (this document), design frames |
| E. Usage page (ORC-15) | `crates/workspace` usage queries with the filters above, `get_usage`, the Usage page and its route with filters | — |
| F. Pull requests page (ORC-13) | `crates/workspace` PR reads (gh), `list_pull_requests`, `get_pull_request`, the page and its routes | — |
| G. Design | Pencil frames: chat replies with in-app and external links; proposal cards for publish, cancel and merge; the new switch in Settings › What automation may do | — |

Each stream keeps to the files it owns; a change another stream owns goes through the coordinator. Every stream
follows AGENTS.md, keeps Windows and macOS equivalent, and ends with an independent review and, for visible changes,
an independent design verification.
