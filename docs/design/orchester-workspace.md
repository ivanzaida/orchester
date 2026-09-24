# Orchester — workspace, agents, connections and access

Date: 2026-09-23. Status: proposed design for review, not an approved implementation contract.

## Brief

Personal agent orchestration first, with a future path to teams. The user requested a convenient, restrained application without bright colors, then added first-class instructions inside Orchester, a discoverable variable catalog, and secrets for staging deployment and remote database access. The user subsequently established that tools own credential bindings; secret settings show read-only links to tools; resources can be imported from another project. The final scope clarification makes the project the owner of all operational resources: tasks/runs, pipelines, agent profiles, instructions, tools, secrets and connections. There is no workspace instruction layer or shared mutable workspace resource library.

Navigation selects a project first. Every functional section uses that project context. All projects is a project-switching entry point; it does not make operational resources global. Imported resources are independent local copies with dependency remapping.

The design is an editable desktop application reference at 1440 × 960. English UI copy, grayscale application surfaces, Inter for interface text and Geist Mono for identifiers and prompts. The user explicitly requested GitHub-like colored code: syntax and diff content now use a restrained GitHub Dark-inspired palette while application chrome remains neutral. No runtime, framework, provider or storage implementation has been selected by this work.

All project names, runs, PRs, credentials, reports, usage events, token estimates and budgets are illustrative fixtures. No repository connection, deployment, database query, approval or secret storage actually occurred.

## Editable source

File: `H:/projects/orchester/data/orchester.pen`, edited exclusively through Pencil MCP.

The root contains nine numbered sections in one horizontal band, ordered left to right with a 320px gutter. Reading order within a flow is left to right, then the next row. Main-flow labels use section.step numbers; alternative states use letters. The layer tree follows the same order.

| Section | Node |
| --- | --- |
| 00 / Foundations | `cuxUu` |
| 01 / Getting started | `K7IL6` |
| 02 / Tasks & runs | `g7ncNr` |
| 03 / Pipelines | `Y6XGt` |
| 04 / Agents & connections | `wNy9i` |
| 05 / Instructions & context | `MRxol` |
| 06 / Tools & imports | `V8h6d` |
| 07 / Secrets & access | `b3N1Ix` |
| 08 / Pull requests | `McWTM` |

Canonical screens and states, in canvas order:

| Screen | Node | Preview |
| --- | --- | --- |
| 01.01 · First launch | `ZBoBm` | [Preview](previews/ZBoBm.png) |
| 01.02 · Authorize GitHub | `jnpRq` | [Preview](previews/jnpRq.png) |
| 01.03 · Project & repository | `W6uXiL` | [Preview](previews/W6uXiL.png) |
| 01.04 · Runner & agent models | `w6ODp4` | [Preview](previews/w6ODp4.png) |
| 01.05 · Ready → Create first task | `x1YpfN` | [Preview](previews/x1YpfN.png) |
| 01.A · Setup blocked / Restore provider access | `Q7htyU` | [Preview](previews/Q7htyU.png) |
| 02.01 · Orchestrator / New conversation | `dOIrp` | [Preview](previews/dOIrp.png) |
| 02.02 · Orchestrator / Task proposal | `I7TCI` | [Preview](previews/I7TCI.png) |
| 02.03 · Tasks / Select drafts to run | `J9467` | [Preview](previews/J9467.png) |
| 02.04 · Create multiple tasks / Review briefs | `OqmXc` | [Preview](previews/OqmXc.png) |
| 02.05 · Run selected tasks / Parallel | `W1Dq6` | [Preview](previews/W1Dq6.png) |
| 02.06 · Run selected tasks / Sequential | `IyJ5a` | [Preview](previews/IyJ5a.png) |
| 02.07 · Task queue / Running and waiting | `Z22wg` | [Preview](previews/Z22wg.png) |
| 02.08 · Project overview | `Twtqc` | [Preview](previews/Twtqc.png) |
| 02.09 · New task / Brief, context & limits | `l2YA4` | [Preview](previews/l2YA4.png) |
| 02.10 · Research running | `RD4pc` | [Preview](previews/RD4pc.png) |
| 02.11 · Review & approve plan | `x3lVC` | [Preview](previews/x3lVC.png) |
| 02.12 · Working task / Conversation | `jgqPe` | [Preview](previews/jgqPe.png) |
| 02.13 · Ready for review | `vpgrJ` | [Preview](previews/vpgrJ.png) |
| 02.A · Paused / Resume from checkpoint | `Oqmog` | [Preview](previews/Oqmog.png) |
| 02.B · Retry limit / Review recovery | `J3JZaT` | [Preview](previews/J3JZaT.png) |
| 02.C · Close with active work / Confirm | `Xz30C` | [Preview](previews/Xz30C.png) |
| 02.D · Close / Waiting for agents to save | `a0tTV` | [Preview](previews/a0tTV.png) |
| 02.E · Next start / Session ended abnormally | `o8DCd` | [Preview](previews/o8DCd.png) |
| 02.F · Runner stopped / Reconnecting | `MAn7V` | [Preview](previews/MAn7V.png) |
| 02.G · Runner reconnected | `YHS1v` | [Preview](previews/YHS1v.png) |
| 02.H · Runner could not restart | `EFY7I` | [Preview](previews/EFY7I.png) |
| Sidebar runner card states (compact) | `O5NoB1` | [Preview](previews/O5NoB1.png) |
| 03.01 · Pipeline / Feature delivery | `Q21HOA` | [Preview](previews/Q21HOA.png) |
| 04.01 · Project connections | `a4Lqb` | [Preview](previews/a4Lqb.png) |
| 04.02 · Add agent provider | `h6CmJ` | [Preview](previews/h6CmJ.png) |
| 04.03 · Agent profiles / Assign connection & model | `WPmtS` | [Preview](previews/WPmtS.png) |
| 04.A · Expired MCP access / Reconnect | `RZUKo` | [Preview](previews/RZUKo.png) |
| 05.01 · Project rules & effective prompt | `yRKeF` | [Preview](previews/yRKeF.png) |
| 05.02 · Resolved prompt / Inspect sources | `fZa3J` | [Preview](previews/fZa3J.png) |
| 05.03 · Variables / Search, inspect & insert | `EfTiT` | [Preview](previews/EfTiT.png) |
| 05.A · Variable unavailable at this event | `m2WjL` | [Preview](previews/m2WjL.png) |
| 06.01 · Project tool library | `ZFwWl` | [Preview](previews/ZFwWl.png) |
| 06.02 · Tool configuration & credentials | `YEWFo` | [Preview](previews/YEWFo.png) |
| 06.03 · Tool access rules | `H97stw` | [Preview](previews/H97stw.png) |
| 06.04 · Import / Select project resources | `W4zGKx` | [Preview](previews/W4zGKx.png) |
| 06.05 · Import / Resolve dependencies | `QqnDd` | [Preview](previews/QqnDd.png) |
| 06.06 · Import / Ready to create local drafts | `hF664` | [Preview](previews/hF664.png) |
| 07.01 · Project secrets | `R99lt` | [Preview](previews/R99lt.png) |
| 07.02 · Add project credential | `tpF7R` | [Preview](previews/tpF7R.png) |
| 07.03 · Runtime approval / One deployment | `nuuZL` | [Preview](previews/nuuZL.png) |
| 07.A · Duplicate secret name | `m9Dhp` | [Preview](previews/m9Dhp.png) |
| 08.01 · Pull requests / Review inbox | `wGtxT` | [Preview](previews/wGtxT.png) |
| 08.02 · Pull request / Request an agent fix | `Rrmhf` | [Preview](previews/Rrmhf.png) |
| 08.03 · Pull request / Verify the agent fix | `i4GUz` | [Preview](previews/i4GUz.png) |
| 08.A · Pull request / Agent answers a question | `qJKSC` | [Preview](previews/qJKSC.png) |
| 08.B · PR updated while composing (compact state) | `hfoDa` | [Preview](previews/hfoDa.png) |
| 08.C · Agent request queued (compact state) | `uD6Ho` | [Preview](previews/uD6Ho.png) |

There are 50 full screen frames; the previous 44 screen IDs are preserved. Screens remain 1440 × 960, at x 48 / 1568 / 3088 with 80px horizontal gaps, 88px row gaps and labels 32px above each frame. Compact recovery panels are counted separately. Section subgroups have a 96px gap.

- Getting started: first launch → GitHub authorization → repository → runner/agents → ready. Setup blocking and compact empty-project feedback are below the main flow.
- Tasks & runs starts with project chat in `I0Ij1T`: new conversation → task proposal. `UIc5y` follows with task library, bulk creation, parallel/sequential scheduling and active queue. The individual flow is in `D4pfd`: overview → new task → research → plan approval → working conversation → ready for review. Pause and retry are alternatives in `O4vgKt`; runner/repository/loading recovery, status variants and interaction notes are in `y8TKW8`. Application lifecycle states (close with active work, runner connection, recovery on start) and the sidebar runner card states are in `Z7Fyr8`. Group positions are y 160 / 1376 / 3640 / 5904 / 7120 / 8216 respectively; the section height is 10832.
- Pipelines: the visual editor is separate from task execution.
- Agents & connections: connections → add provider → assign connection/model in the agent profile; expired-access recovery is below.
- Instructions: rules → composed prompt → variable catalog; unavailable-variable state is below.
- Tools & imports: library → configuration → access policy in the first group. Select → resolve dependencies → ready to import is a separate complete row in `TSGMX`. Success/failure panels remain in `tjNmM`.
- Secrets & access: library → add credential → runtime approval; duplicate-name validation is below.
- Pull requests: `scsYh` contains the review inbox → line-specific fix request → verification on a new commit. `u77pi` contains the question/answer alternative and compact stale-revision and queued-request states. The section is at x 41728 / y 0, width 4896 and height 2592.

Section previews: [Getting started](previews/organization/K7IL6.png), [Tasks & runs](previews/organization/g7ncNr.png), [Pipelines](previews/organization/Y6XGt.png), [Agents & connections](previews/organization/wNy9i.png), [Instructions](previews/organization/MRxol.png), [Tools & imports](previews/organization/V8h6d.png), [Secrets & access](previews/organization/b3N1Ix.png).

## Components and tokens

37 reusable components are defined. Most are in Foundations; the composed secret inspector, effective instruction preview and inline notice have canonical definitions within their owning screens. The shared brand is canonical inside the sidebar. The conversation composer is canonical inside the task panel and reused by project chat and PR feedback. Screens use linked instances; content, icon, state and supported size are intended overrides.

| Family | Node |
| --- | --- |
| Button | `u1XBP` |
| Navigation item | `TSJpn` |
| Status label | `DeizF` |
| Pipeline step | `ajikA` |
| Task row | `zemhy` |
| Select/input field | `ZGwlv` |
| Sidebar | `zo9Py` |
| Tab | `SOW6S` |
| Activity event | `mBkeJ` |
| Evidence check | `l3hyb` |
| Project metric | `dZ2Zt` |
| Run progress step | `i9rVFu` |
| Pipeline summary step | `XVkaB` |
| Key/value property | `tfGRt` |
| Recovery panel | `iN90G` |
| Instruction rule item | `jNTVN` |
| Instruction source row | `SV0BN` |
| Variable row | `ZJ6KN` |
| Secret row | `vxAMD` |
| Secret details inspector | `CQxCb` |
| Tool library row | `sFtAz` |
| Preview execution context | `TnjFo` |
| Effective instruction preview | `RGk5C` |
| Inline notice | `q5LzI` |
| Import selectable resource | `cuocA` |
| Import dependency mapping | `HfNoA` |
| Conversation message | `J5MIT` |
| Conversation composer | `C3nu0z` |
| Orchestrator proposed task | `SGjO0` |
| Task conversation panel | `kjsmd` |
| Agent model assignment row | `kOI6w` |
| Orchester brand | `a0S44m` |
| Task queue row | `clVjO` |
| Task execution mode | `lcwsy` |
| Pull request list row | `QHBNZ` |
| Changed file | `j9NDM` |
| Diff line | `vEZ42` |
| Window title bar (Windows) | `QQmsI` |
| Window title bar (macOS) | `V70DN` |

Colors: canvas `#101010`, sidebar `#141414`, surface `#1A1A1A`, raised `#222222`, hover/selection `#2C2C2C`, border `#363636`, hairline `#2B2B2B`, text `#EEEEEE`, muted `#A3A3A3`, subtle `#8F8F8F`, primary action `#E6E6E6` with `#181818` text. Variables also hold fonts, spacing and 7/12px control/panel radii. Status always includes text and an icon.

## Window chrome

The application draws its own window chrome; the operating system's title bar and window border
are not shown. A 32px title bar in `$canvas` sits above every 1440 × 960 screen frame, so the
window is 1440 × 992. The empty part of the bar is the drag area (double-click maximizes).

- Windows (`QQmsI`): minimize, maximize/restore and close on the right, 46 × 32 each, lucide
  icons at 14px in `$muted`; hover fills the control with `$hover`.
- macOS (`V70DN`): the three window buttons on the left, 12px, 8px apart, 12px from the edge,
  in the platform's standard colors — the only non-grayscale colors in the application.

Screens do not include the bar; design verification compares the screen below the bar with the
screen frame and the bar with the platform component.

## Core workflow

- New task collects a brief, acceptance criteria, context, pipeline and limits in its selected project. Project, agent-role and pipeline/step instructions are included automatically. Repository files are optional context, not the primary rules interface.
- Create & run submits once and displays pending feedback; retry must not create a duplicate run. Draft save and run creation have separate states.
- The sample pipeline is Research → Plan → Implement → Run checks and Review in parallel → Create pull request. Explorer researches, Planner proposes the plan, Builder implements and Reviewer independently reviews. Plan approval occurs before implementation; the user reviews the PR before merging. Failed verification returns to Implement, up to the configured retry limit.
- The task page binds evidence to an exact revision. New code invalidates stale evidence. Review actions expose changes; a report alone is not proof of implemented acceptance.
- Run snapshots retain effective pipeline, agent-profile, model, instruction and tool versions, explicit step overrides and context. Runner loss preserves checkpoints and exposes recovery; it must not manufacture progress or completion.
- Pipelines, profiles, instructions and tools share Draft saved → Publish vN → Published vN. Saving a draft does not publish it. The pipeline editor shows draft v4, published v3 and Run v3; tool setup shows draft v2 and published v1. Publishing affects new runs. Dragging, keyboard selection, undo and persistence remain implementation work.

The task creation fixture includes four acceptance criteria, including cancelled OAuth, and shows the six steps from published Feature delivery v3. Create & run leads to research and planning; it does not bypass plan approval. The plan screen exposes plan v1, scope and acceptance criteria, with Approve plan & continue and Request changes. Requested changes return to Planner; a revised plan requires a new decision.

Pause first settles at a safe checkpoint and stops new scheduling/tool calls; only then may the UI claim Paused. Resume continues the same run and version snapshot. Cancel stops new scheduling and preserves work and artifacts; it does not undo completed external operations, and unresolved in-flight outcomes remain visible. Retry-limit recovery offers one explicitly authorized additional attempt within the remaining budget or revision of the task first. The exhausted example is an alternative Run 129 / Attempt 3; the normal Run 128 shows intermediate checkpoint `4b92ad0` and final reviewed revision `8f3a2c1`.

## Task library, bulk creation and scheduling

The user explicitly requested creating ten tasks and letting agents work on them either concurrently or in sequence. A pipeline defines the steps within one task; the task queue coordinates separate task runs. All of this remains within the selected project.

Tasks (`J9467`) is the main backlog, with All tasks, Drafts, Queue, Needs review and Done tabs. New task opens the existing single-task form; Create multiple opens `OqmXc`. Rows expose task ID, title, published pipeline, status and readiness/dependency, with selection and direct entry to task details/conversation. Run selected opens a scheduling review rather than immediately starting work.

Create multiple supports pasting one title per line, adding rows and editing each task's brief, acceptance criteria, pipeline, limits and dependencies. Pasting titles does not invent accepted requirements. The drawn state shows ten completed briefs, with Task 03 selected. Create 10 tasks creates ten drafts and returns to Tasks; it does not start agents. It remains valid to save incomplete drafts, but missing required details are surfaced before queueing. Creating and queueing must each be idempotent; a failed response must not result in duplicate tasks or runs on retry.

The fixture uses tasks ORC-129–138 in a separate sample backlog from the original overview. ORC-131, Add switcher keyboard shortcuts, depends on ORC-129, Add repository switcher, being merged to main. References to Task 01 before creation are mapped to the new local task identity during creation. All ten tasks use published Feature delivery v3, a $5 per-task cap and 30-minute run limit; the selected group cap is $50. These remain fictional values and records.

| Setting | Proposed behavior |
| --- | --- |
| Parallel (`W1Dq6`) | Start eligible tasks by priority up to the configured task-run limit. The fixture uses two concurrent tasks. A task waiting for plan approval still occupies a task-run slot. Dependency-blocked tasks have not started and occupy no slot; other eligible tasks may proceed. |
| Sequential (`IyJ5a`) | Run one task at a time in the chosen order. The drawn completion gate is Previous task is ready for review. A blocked next task holds this sequence rather than silently skipping ahead. |
| Dependencies | Both modes enforce explicit prerequisites. Ready for review is distinct from Merged to main. A merge dependency releases only after the merge is verified; the dependent task starts from the updated base. |
| Agent capacity | Concurrent task runs and agent slots are separate constraints. A pipeline can itself use several agents. The runner also enforces its available agent slots; insufficient capacity leaves work waiting with a reason. |
| Failure policy | The fixture pauses remaining tasks on an unrecovered task failure. Already active tasks continue; no automatic discard or cancellation is implied. Explicit recovery can retry the failed task or resume the remaining queue. |
| Pause scheduling | Stops new task starts in the selected group. Active task runs continue. Their existing per-task Pause and Cancel controls remain separate. |
| Budgets | Enforce both per-task and group caps. Exhausting the shared cap stops further scheduling and exposes a budget reason; the exact accounting and in-flight settlement remain runtime engineering work. |

Queue 10 tasks schedules the reviewed group with pinned configuration versions. Each task retains its own run, conversation, branch and result. The group does not become one shared agent conversation. Plan approvals and tool access decisions come from each pipeline and tool policy; bulk scheduling does not bypass them. Separate branches/workspaces are required for concurrent edits; reviewed PRs and explicit merges determine integration order. Parallel execution does not guarantee conflict-free merging.

The active queue (`Z22wg`) demonstrates Group 07 with two tasks researching, seven waiting for concurrency and one blocked on merge. Each waiting row explains why it has not started. Edit queue changes pending order/limits; reductions in concurrency drain through existing work rather than cancelling it. Changes to already-started run snapshots require a separate explicit operation. Queue-tab counts include all unfinished scheduled work, including running and dependency-blocked tasks; the status summary disambiguates those counts.

Preflight must validate required briefs, available published configuration, authorization, runner readiness, dependency cycles and impossible sequential order. Invalid work stays intact with actionable reasons; any partial queueing must be an explicit selection, not a silent skip. Self/cyclic dependencies are rejected. A prerequisite outside the selected group must be visibly identified and must not be automatically started without selection. If a prerequisite fails, is cancelled or its merge is reverted, dependent work remains blocked for review.

Interaction notes: select-all applies to the current filtered set and displays its count. Task-row activation opens details; the checkbox only changes selection. Drag reorder also needs keyboard Move up/down actions and one undoable reorder. Paste a task list is an explicit user action with a reviewable result. Closing retains the batch draft; destructive discard is separate. Disabled/pending/error states, live scheduling and delivery remain implementation work. Proposed strings extend `tasks.bulkCreate.*`, `tasks.selection.*`, `taskQueue.*`, `taskDependencies.*` and `runGroups.*`.

## Project orchestrator conversation

Orchestrator is a permanent project navigation entry between Overview and Tasks. It opens a project conversation without requiring a task. New conversation starts a separate discussion; History finds prior discussions within the selected project. The empty composition state is `dOIrp`; the ten-task proposal state is `I7TCI`. These are proposed product interactions, not an implemented chat service.

The project chat handles ideation, decomposition, queue coordination and cross-task progress questions. A task's Conversation stays scoped to that task/run, with Orchestrator or a participating agent as recipient. Its panel now explicitly says Task conversation and To: Orchestrator · this task. Opening a task or project chat preserves the other conversation and its draft. A project message is not silently broadcast into every agent's task context.

The header identifies the project Orchestrator profile and model; Agent profile links to the existing profile editor. Project instructions apply to the project coordinator, and Manage context shows attached files, selected tasks and applicable source versions. Chat text alone does not publish permanent instructions. Repository access is not proof the entire repository was read; responses must identify sources actually consulted. Tool credentials and raw secrets remain excluded. Provider/model names in this fixture have the same unverified illustrative status as the other model screens.

The shown user asks to plan from an attached scope.md. The Orchestrator proposes ten briefs with acceptance criteria, a merge dependency, parallelism of two tasks and a $50 aggregate cap. The proposal's local task numbers are not persisted task IDs. Review & create tasks opens the existing bulk brief editor (`OqmXc`) populated with that proposal. Selecting a proposed row opens its brief in that editor. Create 10 tasks then saves drafts and returns confirmed IDs/links to this conversation; it does not queue work. Run selected opens the existing parallel or sequential scheduling review with the proposal settings carried over. Edits preserve the conversation/proposal link. This is one connected flow, not a second task or scheduler implementation.

Natural-language queue commands use the same project actions, permission checks, scheduling limits and plan/tool approvals as direct UI controls. Resolve the intended task/group scope; ambiguous requests need clarification. An explicit authorized action need not acquire an unrelated extra chat approval. Changes and their outcomes remain inspectable with exact affected task links. Requests to pause scheduling hold new starts; pausing a working task is a separate action at its checkpoint. Progress answers read actual run state and link to tasks; an unavailable status source is reported as unavailable. Fresh authorization, missing credentials or a changed configuration cannot be bypassed through chat.

The composer is the existing `C3nu0z` promoted to Conversation / Composer and reused by linked references; `SGjO0` is the new Orchestrator / Proposed task row. Existing message, button, status, property and navigation components are reused. The composer is disabled while empty. Suggestions populate an editable prompt rather than dispatching an implicit operation. Enter adds a newline; Ctrl/Cmd+Enter sends. Escape closes History/context overlays and restores focus. History and context overlays are specified entry points, not additional drawn screens.

Proposed runtime states: preserve drafts on failed sends and show Retry; deduplicate message and action retries; distinguish Sending, Thinking, Delivered and action-confirmed outcomes. A response failure must not erase the user's sent message or existing proposal. Stop response only stops response generation, not running tasks. Provider access failure links to Connections while retaining the conversation. Loading must not impersonate a new empty conversation. Revalidate proposal/configuration versions before creation or launch; never apply stale settings silently. Conversation retention, context compaction and history search need an engineering contract. Proposed copy namespaces extend `orchestrator.*`, `conversation.history.*`, `conversation.context.*` and `orchestrator.proposal.*`.

At narrower widths the task proposal becomes an on-demand panel above the composer, keeping chat usable; collapse project navigation first. Keyboard focus, panel transitions, persistence, response streaming and actual routing require runtime acceptance tests. [Project chat overview](previews/organization/I0Ij1T.png).

## Task conversation

The task has a visible Conversation tab, an Open conversation entry on the review summary, and a conversation panel beside running/paused progress. The recipient defaults to Orchestrator; the picker can target a participating agent directly. Orchestrator routes messages, while direct messages retain their selected recipient. Messages belong to the task and record run, step, recipient and author.

Delivery states distinguish Queued, Delivered and Acknowledged. Acknowledgement does not prove the requested change was applied. Work-changing messages are delivered at the next checkpoint; Pause is the visible control for an urgent interruption. While paused, messages remain queued and are delivered before the next action on resume. Scope changes return to plan review rather than silently changing approved work.

Proposed interaction contract: failed or pending sends retain the draft; retries deduplicate the message; Enter adds a newline and Ctrl/Cmd+Enter sends. Escape closes the recipient picker and restores focus. Keyboard, delivery and persistence behavior remains to be implemented and tested.

## Application lifecycle and runner connection

These states express the lifecycle in [storage.md › Lifecycle](../architecture/storage.md#lifecycle): closing the window closes the application, the runner and agents cannot outlive it, and nothing restarts automatically after the application itself ended; runs interrupted by a runner drop while the application is open continue automatically after reconnecting. They sit in section 02, group `Z7Fyr8` ("States · Application lifecycle"), over the project Overview (`Twtqc`) background. [Group overview](previews/organization/Z7Fyr8.png).

**Close with active work (`Xz30C`).** The close request is vetoed and a modal opens. Title: *Close Orchester while agents are working?* Explanation: closing stops the runner and every agent; there is no background mode. It lists the tasks with an agent at work (task key, title, current step · agent role, Running status, provider · model), reusing `clVjO` Tasks / Queue row without its selection and open cells. Tasks that only wait for input are named in a note and are unaffected. A short block explains both exits:

- *Wait for agents to save* (primary, right): no new steps or tool calls start; each agent continues to its next checkpoint, runs are marked paused, then Orchester closes. "This can take a few minutes."
- *Quit now* (secondary, isolated on the left with a power icon): stops runner and agents immediately; work after each run's last checkpoint is not recorded as progress; uncommitted changes stay in the worktree and are shown on next start. It is grayscale; the distinction is wording, position and the explanation, not color.
- *Cancel*: keeps the app open, nothing changes.

**Waiting (`a0tTV`).** Same modal, title *Saving before closing*. Summary "1 of 2 runs saved"; each row's status moves from *Saving at next checkpoint…* (loader) to *Saved* (check) with "Paused at 9d07b3f". *Quit now* stays available; the primary becomes a non-interactive "Closes after 1 more run" indicator; Cancel is not offered once the runner has stopped scheduling (proposed; pending product confirmation, since cancelling here would leave the runs paused without closing).

**Runner connection.** The runner can drop; the app reconnects automatically. Connection problems appear as an inline notice (`q5LzI`) at the top of the Overview body and in the sidebar runner card; interrupted runs show an *Interrupted* status (pause icon) with their last checkpoint in the task list, "Running now" drops to 00 with "2 runs interrupted", and the activity feed records the runner events. Runs interrupted by a runner drop continue automatically from their last checkpoint once the runner is back (user decision, 2026-09-24).

- *Reconnecting* (`MAn7V`): "Runner stopped unexpectedly — reconnecting (attempt 2 of 5)". "The 2 active runs were interrupted and keep their last checkpoint. They continue from it automatically once the runner is back; new work waits until then." Trailing: "Next attempt in 8 s", *Open runner log*.
- *Reconnected* (`YHS1v`): "Runner reconnected". "The 2 interrupted runs continued from their last checkpoint." Auto-dismisses ("Closes in 5 s", with a manual close). The task rows in this frame still show the interrupted state and need updating to running.
- *Could not restart* (`EFY7I`): "Could not restart the runner". "Tried 5 times since 14:02. Interrupted runs keep their last checkpoint. Nothing runs until the runner starts." Actions *Open runner log* (opens `.orchester/logs/runner-<date>.log`) and *Restart runner* (primary). Persistent until resolved.

Sidebar runner card (`O5NoB1`), always icon + label: *Starting local runner* / "Agents can start when it's ready" (loader); *Local runner online* / "2 of 4 agent slots in use" (existing dot); *Reconnecting to runner* / "Attempt 2 of 5 · runs interrupted" (refresh); *Runner offline* / "Could not restart · see runner log" (unplug). The card states are overrides of the existing sidebar card inside `zo9Py` instances; no new component was added.

**Session ended abnormally (`o8DCd`).** On the next start after *Quit now* or a crash, a modal: "The last session ended before agents could save". "Orchester stopped unexpectedly on Sep 23 at 14:12 and three runs were interrupted. Nothing has been restarted. Choose what to continue: each run continues from its last checkpoint, and work after it is not counted as done." A table lists each interrupted run: task (key, title, project · run), *Interrupted during* (step, agent role · provider · model), *Reason*, *Last checkpoint* (commit, time saved), *Worktree* ("3 files not committed" with View changes, or "No uncommitted changes") and a per-run *Continue*. The runs may come from several open projects. A run stopped during an external tool call says "outcome unknown", and a notice asks the user to check the target before continuing, because continuing repeats the step from its checkpoint. Footer: *Open runner log*, *Decide later* (runs stay interrupted and listed on their task), *Continue all 3 from checkpoints* (primary). Uncommitted changes are never discarded silently. After *Wait for agents to save*, no dialog appears; runs are simply paused.

Proposed copy namespaces: `lifecycle.close.*`, `lifecycle.recovery.*`, `runner.connection.*`, `runner.card.*`. Retry count (5), backoff and the auto-dismiss delay are illustrative values. All seven states were checked with full-size export review and depth-14 layout checks. The only remaining findings are disabled "open task" cells in the queue-row instances in the two close dialogs, which the checker reports at stale positions; they are not rendered.

## Pull request review and agent feedback

The user requested viewing pull requests and leaving comments for an agent to fix code or answer a question. Pull requests is now a project navigation entry after Tasks. It lists PRs from the selected project's repository with linked task, feedback state, current head and agent verification. Ready for review (`vpgrJ`) now opens Review PR in Orchester; Ask agent opens general feedback for the linked PR. Open in GitHub remains a clearly separate external destination.

The new inbox (`wGtxT`) shows three illustrative open PRs: #42 needs review, #43 has an agent fixing feedback, and #44 is not linked to an agent task. An unlinked PR remains readable; link or create an authorized project task before asking an agent to execute work. PR open/draft/merged/closed state is distinct from agent readiness and local verification. Refresh errors preserve existing data with a stale marker and last successful sync time; loading and empty results must not fabricate PRs. Connection/permission failures lead to the relevant project Connection. Repository lookup, GitHub API synchronization and access have not been implemented or verified by these screens.

The diff view (`Rrmhf`) includes a changed-file list, selected revision range, unified code diff, Viewed state and a contextual feedback panel. Code additions/removals use plus/minus signs, line numbers and muted green/red surfaces; a comment selection uses blue, so meaning is not dependent on color alone. The selected fixture shows new lines 43–45 of src/auth/github.ts at 8f3a2c1. The full PR totals six files and +248 / −32; the shown handler is one excerpt, not the entire diff. The test counts are illustrative agent/local evidence, not a claim about GitHub Actions or required branch checks.

Code styling follows the color roles in the official [GitHub theme](https://github.com/primer/github-vscode-theme/blob/main/src/theme.js), adapted to the existing application. Semantic tokens: code-bg #0D1117, code-text #E6EDF3, code-keyword #FF7B72, code-string #A5D6FF, code-function #D2A8FF, code-variable #FFA657, code-constant #79C0FF and code-comment #8B949E. Diff additions use #14261C with #7EE787 markers; removals use #301C20 with #FFA198 markers; selected ranges use #1B3652 with a #58A6FF edge. The application navigation, forms, buttons and feedback panels remain grayscale. These are design tokens inspired by GitHub Dark, not a claim of pixel-identical GitHub rendering.

The shared diff-line component now contains colored text spans with the same 12px Geist Mono typography and original whitespace; line content is retained in metadata for inspection. All 39 displayed code lines across the three review screens were checked by concatenating rendered token contents against the original strings. A linked component specimen in Foundations (`z8v8m7`) covers comment, deletion, addition and selected-addition states. This is editable design content, not a production tokenizer: implementation needs a language-aware syntax highlighter, safe text rendering and correct tab/Unicode/long-line handling. No runtime highlighter was implemented or selected.

Agent feedback has explicit Ask a question and Request a fix modes. Both record project/repository, PR identity, selected base/head, file, diff side, line/range, source excerpt, author and target agent. General PR feedback omits the line anchor but retains the revision context. The target selector defaults to the linked task's Builder profile and displays its model and task; runtime must resolve the actual profile and permissions, not infer an agent from a GitHub author name. Questions request an explanation without changing code. A fix request is an explicit instruction to continue the linked task on its existing PR branch, preserving the applicable run configuration and approval policies.

Proposed visibility choice: the shown comments are Orchester-only and this is labeled beside every thread and before sending. They are not GitHub review comments, do not post to GitHub, and do not constitute a GitHub Request changes review. GitHub discussions, formal approval and merge remain available through Open in GitHub. Native GitHub discussion import/posting is outside this increment and would need explicit source/visibility and delivery states. No actual GitHub comment, review, task execution or push occurred during this design work.

Sending preserves a local draft until persistence is confirmed and deduplicates retries. Distinguish Saved/Queued, Delivered, Working, Answered, Ready to verify and Resolved; a successful send alone must not claim execution. Questions can fail or need clarification without becoming code work. A queued request (`uD6Ho`) links to the existing task queue. A fix respects task concurrency, agent capacity, branch/worktree serialization and the remaining task budget; multiple requests must not create concurrent writers on one PR branch. The sample shows $3.72 remaining from the existing $5 cap. If budget, time, access or agent availability prevents execution, show the actual blocking reason and retain the request. Previously finished tasks can return from review to implementation; never create a duplicate task or silently reset limits.

On completion (`i4GUz`), the thread links the agent's answer, pushed commit b7c92e4, changed files and checks for that exact revision. Since your review compares 8f3a2c1 with b7c92e4 and shows two updated files (+4 in the handler, +12 in its test), while the PR still contains six changed files in total. The original 43–45 anchor maps to current 47–49; the added cancellation guard is highlighted separately. The agent reports what it did, but the thread remains Ready to verify. Mark resolved is the reviewer's action and closes only the Orchester thread; it does not approve or merge the PR. Reply or request another fix keeps the conversation and source references together. Resolved threads remain inspectable and can be reopened.

The question state (`qJKSC`) is an alternative first-thread scenario on the original head, not a second event after the fix. It shows a question about lines 48–49, the Builder's explanation and a source link. No code changed and no implementation run was requested by that question. Follow-up questions remain questions; Request a fix explicitly changes the requested action. Answered does not silently mean Resolved.

New commits never discard comments or move them to unrelated code. Track anchors against immutable revisions and explicitly identify a mapped/current or outdated location. If the PR changes while a fix request is drafted (`hfoDa`), preserve the draft, expose the new head, and review/reanchor before sending work against it. A question about historical code can retain its original revision. Force-push, deleted files, renames, binary/large/generated files, repository permission loss and merged/closed PRs need clear handling; do not invent a mapping or write to an obsolete branch. Revalidate the head before applying work and serialize external updates with runner changes. Viewed markers and evidence apply to known content/revisions; modified files and new code require renewed review. Preserve the original review baseline when incoming updates arrive.

Keyboard requirements: file search, file selection, focusable diff lines, keyboard range selection and a visible Add feedback action; Escape closes the contextual composer while retaining the draft and restores the selected-line focus. Ctrl/Cmd+Enter sends, Enter inserts a newline, and empty input disables sending. Pending/failed sends expose progress or Retry without duplicating work. At narrower widths the file list collapses and feedback opens as a panel, leaving the diff readable with deliberate horizontal scrolling for long code. Screen-reader labels must include old/new line, change type and thread state.

The three new reusable components are QHBNZ (PR list row), j9NDM (changed file), and vEZ42 (diff line). Messages, composer, navigation, tabs, status, properties, notices and recovery panels reuse existing linked components. Proposed copy namespaces: `pullRequests.*`, `pullRequests.diff.*`, `pullRequests.feedback.*`, `pullRequests.revisions.*` and `pullRequests.recovery.*`. Source code, review text and repository paths are user/repository data. [Review-flow overview](previews/organization/McWTM.png).

## Agent profiles and connections

An agent profile defines a role, responsibilities, role instructions and a project connection/model. Connections establish project access to repositories, agent providers and MCP servers. Tools expose the actual callable actions. A provider is selected per profile; the project does not impose one provider on every agent.

The mixed-provider fixture assigns Explorer and Reviewer to Claude · Kontur / Opus, and Orchestrator, Planner and Builder to Codex · Kontur / Sol. The Builder inspector contains separate Connection and Model selectors; provider-supported settings such as reasoning/context appear conditionally. Pipeline steps select a published project profile and inherit its model by default. A step can explicitly override the model, and the inspector shows the effective connection and model.

Connections (`a4Lqb`, section 04) lists GitHub, Codex · Kontur, Claude · Kontur and Project knowledge MCP. The selected connection shows provider, execution location, authentication source, status, last check, available models and read-only Used by links. Add connection (`h6CmJ`) configures an agent provider and validates access before saving. Expired MCP access (`RZUKo`) exposes Reconnect and preserves task work. Revocation blocks future calls immediately; ordinary profile publishing does not rotate or revoke credentials.

Sol, Luna and Opus are illustrative labels supplied during the design discussion. Provider adapters, public model identifiers, authentication options and supported settings were not verified by this design task. The implementation must obtain actual model catalogs and capabilities from each adapter and validate profile compatibility; the mockups do not establish working Codex or Claude integration.

## First project setup

First launch (`ZBoBm`) appears when the personal workspace has no projects or resumable setup. It has no selected project or project navigation, and no fabricated recent projects, activity or connected-runner status. Create your first project opens the first setup step. The name field initially starts empty; the following setup fixtures show the state after the user enters Kontur. The three-step preview explains repository access, mixed-provider agent assignment and the first task. The primary action creates a setup draft once, rather than starting agents or triggering provider authorization by itself. Existing incomplete setup should resume at its saved step; a configured returning workspace opens its projects rather than repeatedly showing Welcome.

Setup covers project/repository → runner and agents → readiness. GitHub authorization precedes repository selection; unavailable repository/continue controls stay disabled. The second step configures the runner and each role's connection/model, including mixed providers. The final review checks repository, runner, required profiles, instructions and the starter pipeline. Finish & create first task publishes the reviewed starter configuration and opens New task; it does not start a run automatically.

An expired required provider blocks completion with Reconnect or change-profile recovery. Save & leave retains incomplete setup so it can be resumed. Connection authorization is scoped to this project, including when importing configuration from another project.

## Instructions

Instructions are stored and versioned within a project in Orchester. Scope navigation covers project rules, agent roles, pipeline overrides and step overrides. A rule can target selected agent roles and an injection event. The mockup selects agent start/resume; the final event vocabulary remains to be specified. Importing an instruction from another project creates a new locally owned definition rather than a shared rule reference.

The preview displays proposed assembly order within the selected project: agent role → project rules → pipeline instructions → step instructions → task context. This order is an inspectable composition, not a promise that arbitrary contradictory prose can be resolved automatically. Inherited rules should retain source and version; disabling or replacing one must be explicit. Runtime permissions and budgets are separate enforced controls, not prompt text.

Publishing a draft creates a new version used by new runs. Existing runs retain their snapshot. Applying a new version to an active run needs a separate explicit action at a defined boundary; it is not designed as a silent update.

View full prompt opens the source-indexed read-only viewer. Variables are resolved for the selected task/agent/injection point. Copy full prompt copies composed instructions while excluding secret material. Unknown variables and missing required sources receive actionable validation. Closing with Escape restores focus to the invoking control and retains edits.

The Builder preview now accounts for seven individual sources: role, Implementation standards draft v3, Git conventions v1, Communication style v1, pipeline v3, step v2 and task context. Review checklist v2 is explicitly excluded because it targets Reviewer. The same execution context appears in the preview, full viewer and variable catalog: task ORC-128, historical Run 071 / Attempt 1, Builder / Implement, agent start before the first commit, with draft substitution identified. It is a historical preview context, separate from the current working Run 128. A task summary links to per-agent snapshots because applicable source counts differ by role.

## Variable catalog proposal

The Variables control is visible beside the prompt editor. The catalog searches names and descriptions, groups variables by source, and shows type, explanation, current value and availability. Selecting a variable does not modify the prompt; Insert writes its expression at the retained cursor and creates one undoable edit. Escape cancels insertion. Lists need a defined rendering format before implementation.

The count of 24 in the mockup refers to this proposed catalog, not a discovered runtime API:

| Group | Variable | Type | Availability |
| --- | --- | --- | --- |
| Task | `task.id` | string | After task creation |
| Task | `task.title` | string | Task selected |
| Task | `task.description` | string | Task selected |
| Task | `task.acceptance_criteria` | list of strings | Task selected; may be empty |
| Task | `task.attachments` | list of safe attachment references | Task selected; may be empty |
| Task | `task.created_at` | timestamp | After task creation |
| Project | `project.id` | string | Project selected |
| Project | `project.name` | string | Project selected |
| Project | `project.repository` | sanitized repository reference | Repository connected |
| Project | `project.default_branch` | string | Repository metadata loaded |
| Agent | `agent.id` | string | Agent assigned |
| Agent | `agent.name` | string | Agent assigned |
| Agent | `agent.role` | string | Agent assigned |
| Agent | `agent.model` | string | Model selected |
| Pipeline | `pipeline.id` | string | Pipeline selected |
| Pipeline | `pipeline.name` | string | Pipeline selected |
| Pipeline | `pipeline.version` | integer | Published version selected |
| Run | `run.id` | string | Run created |
| Run | `run.attempt` | integer | Attempt created |
| Run | `run.budget` | decimal in project currency | Run limits configured |
| Run | `run.branch` | string | Working branch created |
| Run | `run.revision` | string | Commit available |
| Step | `step.id` | string | Step selected |
| Step | `step.name` | string | Step selected |

The sample syntax is `{{task.title}}`. Variables are evaluated as data, not executable expressions. Secret values are excluded from this catalog. Missing values must not silently become empty text: explain the unavailable phase and offer an appropriate injection point or an explicitly configured fallback.

The unavailable-variable frame demonstrates `run.revision` before the first commit. Required policy blocks injection; the user can choose a later injection event or explicitly configure a fallback. The UI must not fabricate a revision or silently insert an empty string.

## Project tools and imports

Tools, secrets, instructions, agent profiles, pipelines and connections are all project-owned. The navigation entry opens an explicit project context; the breadcrumb and project selector show the current owner. A pipeline selects tools and profiles available in its project. Connections represent provider/server authentication within the project; Tools represent concrete callable actions. Their exact backend relationship remains a separate engineering contract.

The tool library offers creation of HTTP, command and database tools and discovery/import from an MCP connection. Setup owns action configuration, input/output mapping, credentials, allowed environment, access rules and version. The sample HTTP deployment tool selects `STAGING_DEPLOY_TOKEN` from Kontur and maps it to bearer authentication. The example deployment endpoint is fictional. Configuration validation must not execute a deployment. A future test action must clearly expose its actual side effects and apply the normal access policy.

A tool may only bind secrets owned by its project. Tool settings offer New project secret, returning the newly created reference to the binding selector. Secret settings have no tool picker or editable tool policy: Used by is a derived read-only list linking to the consuming tool. The secret creation screen collects name, type, value and environment in its current project.

Import resources is available in project navigation and from resource libraries. It covers pipelines, agent profiles, instructions, tools and secrets. The proposed default is an independent copy with a new local identity and source provenance. Dependencies must be enumerated: pipeline steps reference local agent/tool/instruction versions, and tools reference local connections and secrets. Tool credential dependencies are mapped to an existing compatible target-project secret, imported as a new secret, or supplied with a new value. Standalone secret import is also supported. Connections require target-project authorization before use; importing configuration must not silently confer access. No credential value is displayed in the preview or placed on the clipboard.

The import fixture uses Select resources → Resolve dependencies for Preview release v3 from Personal website into Kontur. Seven mapping rows show the pipeline and six dependencies: reuse local Builder published v2 and Implementation standards v2; copy Build preview; rename the copied Deploy preview to Website preview because Kontur already contains a draft Deploy preview; import PREVIEW_HOST_API_KEY; and authorize Preview host configuration in Kontur. The library shows that existing draft among five tools, with two existing secret bindings.

The result is four new resource copies, two reused local resources and one separately authorized connection. The blocked and ready states differ in destination authorization; source access is never copied. The target policy is explicit: Build preview → Build, Website preview → Deploy, Preview environment, each deployment requires approval, all other callers denied. New configuration stays draft until reviewed and published; import does not start a run. Source/version and destination mappings remain inspectable.

Source edits do not propagate. Permission to read/import source resources and create target resources must be checked independently. Name collisions require an explicit rename or reuse selection, never a silent overwrite. Missing credentials leave the tool unready. Import failure preserves mappings and the original project and allows an idempotent retry without duplicate resources. Success opens the imported pipeline draft for review.

## Secrets and access

The proposed design stores credentials separately from prompt rules and task context. A secret has an owner project, environment, name, type and value. The tool owns credential references, operation constraints and step access rules. Values are masked in management UI; replacing a value rotates it. Used by is a read-only reverse reference to local tools. No actual secrets exist in this design file or its previews.

Agents request capabilities through named tools. The tool receives the credential for the allowed operation/window; the model must not receive raw secret values through prompts, variables, copied instructions, logs or tool output. Implementation must keep credentials out of agent-readable process state and redact tool output; a mask in the UI is not a security boundary. Storage encryption, key management, runner isolation, provider adapters and rotation are open engineering decisions requiring separate review.

The sample database capability uses a database account with read-only privileges and a constrained query tool. The deployment tool binds a token and grants its `deploy.staging` capability to a specific step. Read-only guarantees require database/tool enforcement, not instructions asking the agent to avoid writes. Rotation and archival remain secret lifecycle actions; usage remains observable from the secret, while editing permissions happens in Tools.

The Access tab (`H97stw`) edits the allowed pipelines and steps, environment, approval policy, access window and denied behavior. Configured Used by links are distinct from the allowlist. The selected policy asks for each deployment; policy configuration may permit previously authorized scopes without repeated prompts. The shown deployment approval names project, environment, run, step, action, exact revision, tool and credential reference. It authorizes one deployment and expires when the step finishes. A different revision or operation requires a fresh policy decision. Denial leaves the step awaiting a changed plan; it does not imply a crash or erase work. All other callers are denied. Revoke access is a separate immediate action.

Add-secret's successful fixture uses the unique name STAGING_READ_API_KEY. The duplicate state shows STAGING_DEPLOY_TOKEN already exists in Kontur / Staging, disables Save and links to the existing secret. Replace value remains the distinct rotation action.

Audit records contain who/what requested access, capability, scope, run/step, decision, time and outcome without credential values. Revocation affects future calls immediately; completed external side effects are not reversed. Network failure must not display a successful save, deployment or query. Retries need idempotency and explicit outcome handling.

## Copy, layout and implementation acceptance

Proposed i18n namespaces: `nav.*`, `workspace.*`, `tasks.*`, `runs.status.*`, `runs.controls.*`, `planReview.*`, `conversation.*`, `pipelines.*`, `agents.*`, `connections.*`, `setup.*`, `resourceVersions.*`, `instructions.*`, `variables.*`, `tools.*`, `projectImport.*`, `secrets.*`, `access.*`, `recovery.*`. Examples: `instructions.publishVersion`, `instructions.previewEffective`, `variables.insert`, `variables.unavailableAtPhase`, `tools.credentials`, `tools.validateConfiguration`, `projectImport.resolveDependency`, `projectImport.independentCopies`, `secrets.usedBy`, `secrets.add`, `secrets.replaceValue`, `access.allowDeployment`, `access.deny`, `agents.inheritModel`, `connections.refreshModels`, `conversation.queuedAtCheckpoint`, `planReview.requestChanges`, `setup.resume`. Prompts, task descriptions, secret names and repository content are user-authored data.

Desktop is verified visually at 1440 × 960. At narrower widths collapse navigation first, then open inspectors on demand; stack creation-form columns. Dialogs need viewport-constrained scrolling and focus trapping. Search, pagination, keyboard navigation, Escape, clipboard, screen-reader labels, loading, validation and persistence still need runtime implementation and testing. Native/web runtime choice and mobile editing are not established by these screens.

Visual evidence: the original 13 screens and 20 additional screens were inspected through live Pencil nodes and screenshot/export review. The independent audit was repeated against the expanded 32-screen set; its remaining import/library fixture mismatch was corrected by adding the existing draft Deploy preview and updating the count. The subsequently requested first-launch screen received primary-agent layout and full-size visual review. Full-size export review caught and corrected a tool-list overlap and import-tab spacing that the structural check did not flag. Repeated controls use 30 linked component definitions. See the [audit and resolution record](../evidence/design-consistency-audit-2026-09-23.md).

The subsequent canvas-organization pass grouped the same 33 screens into eight numbered sections, aligned the layer tree and screen labels, and separated normal flows from alternate states. All 33 screens passed depth-9 layout checks after relocation; section exports were inspected for actual visibility and ordering. Reflow exposed excess tool-list height, which was corrected in the library and three import backgrounds. The product flows and screen IDs were preserved.

The task-management extension added five screens for the ten-task backlog, bulk creation, parallel scheduling, sequential scheduling and the active queue. Each received a full-size visual review and passed depth-9 layout checks; Foundations and the expanded section hierarchy also passed. That extension brought the design to 38 screens and 32 reusable components. It received primary-agent design verification, not another independent audit. [Task-management overview](previews/organization/UIc5y.png).

The subsequent project-orchestrator extension added the new-conversation and task-proposal screens, bringing the design at that point to 40 screens and 34 reusable components. Both new screens were inspected at 1440 × 960. The shared sidebar and task-chat labeling were checked visually on the working-task and Instructions screens; all 40 screens plus Foundations passed depth-9 layout checks. That root layout retained eight numbered sections. The new chat screens sit first in section 02; old screen IDs remain stable and the numbered labels were updated. This was primary-agent verification, not a new independent audit.

The PR-review extension adds four full screens and two compact recovery states in section 08. All four screens and both compact states received full-size visual inspection; the updated task-review entry point and section overview were also inspected. A clipped footer on the fix-verification panel was corrected by reducing its layout gap and rechecked. All 44 full screens, Foundations and the PR alternate-state group passed depth-9 layout checks; root layout passed. The shared sidebar now includes Pull requests. The current document has nine numbered sections and 37 reusable components. This was primary-agent design verification; execution, GitHub synchronization, comment persistence and diff anchoring remain unimplemented specifications.

The subsequent colored-code request updated the same three diff screens and shared diff-line component without changing the fixtures or screen IDs. All three screens and Foundations passed depth-11 layout checks after token sizing was corrected; final full-size exports were inspected. Code-only color exceptions are now part of the user-selected visual direction. [Syntax and diff states](previews/z8v8m7.png).

These are static design checks. Actual orchestration, provider authentication, chat delivery, persistence, responsive behavior, accessibility and credential isolation were not implemented or tested. No production code or real external service state was changed by this design work.
