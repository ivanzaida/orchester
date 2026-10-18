# The Orchestrator's toolset

Status: decided by the person on 2026-10-15; decision 5 and the waits changed by the person on 2026-10-17 (ORC-42).
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
     Orchestrator merge runs* it merges at once, only a run the person asked it to merge. The merge keeps all of
     `merge_run`'s rules: only a task's latest run that succeeded, only when the main checkout needs no local
     integration, else refused (`needs_decision`, `merge_failed`); conflict choices and approving the merged result
     stay the person's, on the Local integration page. The merge is kept in the conversation as an action.
   - Both switches are off until a person turns them on; synthetic input cannot flip them (the same rule as *Let
     automation change agent permissions* and *Let the Orchestrator publish drafts*).
   - **Never from the chat:** answering approvals, deciding plans, widening permissions, secret values and deleting
     work (worktrees, drafts, tasks).
6. **Links everywhere.** Every object a tool returns carries an app `link`. The chat renders links and a click opens
   the place in the app.

## Tools

`existing` tools are already in the app's MCP server; `new` ones are built by the work streams below. All reads take
an optional `project` (id or name); without it, list tools answer for every project with each item naming its
project.

| Area | Read | Change (draft / free) | Proposed (person clicks, or switch) |
|---|---|---|---|
| Projects | `list_projects`, `get_project` (existing) | — | — |
| Tasks | `list_tasks`, `get_task` (existing); `get_conversation` (new, ORC-42: a task's conversation as its Conversation tab shows it, paged) | `create_task`, `update_task` (existing) | — |
| Runs | `get_run`, `get_changes` (existing); `list_runs` (new: a task's or project's runs, newest first, with phase and links) | `pause_run`, `resume_run` (existing) | `run_task` (direct in auto mode), `cancel_run`, `merge_run` (direct with *Let the Orchestrator merge runs*) |
| Waits | `list_approvals` (existing); `list_questions` (with the steps waiting for a message and what their agents wrote last), `get_plan` (new, read only) | — | `answer_question`, `send_message` (direct with *Let the Orchestrator answer agents*) |
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

Deciding approvals (`decide_approval`) and plans (`decide_plan`, and a `send_message` that would request a plan's
changes), widening what agents may do, secret values, project setup, connections' changes, MCP server changes,
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
