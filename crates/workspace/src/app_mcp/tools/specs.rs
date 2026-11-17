//! The app's tools as clients list them: each one's name, what it does and its input schema. A static table,
//! kept apart from how a call reaches a tool ([`super::call`]).

use std::sync::Arc;

use rmcp::{handler::server::common::schema_for_type, model::JsonObject};

use super::{
  across, approval_settings, approvals, continue_run, conversation, daily_report, decisions, delivery, limits,
  messages, module_delivery, modules, pipeline_copy, plan_read, plans, projects, questions, queue_hold,
  return_to_draft, run_list, run_task, runs, standing, task_write, tasks, usage, watches, worktree,
};

/// One tool as clients list it.
pub(super) struct Spec {
  pub(super) name: &'static str,
  pub(super) description: &'static str,
  pub(super) schema: fn() -> Arc<JsonObject>,
  /// It reads and changes nothing.
  pub(super) read_only: bool,
}

pub(super) const SPECS: &[Spec] = &[
  Spec {
    name: "list_projects",
    description: "The projects of the workspace, most recently opened first: id, name, folder, whether each uses the code index, its limits (token budgets, default time limit, agents working at once), and its link in the app (its Overview). Links are the app's route paths; the person opens one with a click.",
    schema: schema_for_type::<projects::ListProjects>,
    read_only: true,
  },
  Spec {
    name: "list_tasks",
    description: "A project's tasks, as its task list finds them: key, title, workflow status, priority, story points (points, set_by person or automation, automation's reason; null for none), tags, blocked_reason (the manual Blocked's reason, or null), held_by_blocking_tasks (whether unfinished blocking tasks keep it from running), pipeline, where the task stands, and its link in the app (its page). Without `project`, every project's, each naming its project, as stored: git is asked which runs were merged outside Orchester only for a project named; a team project's tasks are read from its team server, as a call naming it gives them; unreadable_projects lists the projects that could not be read, with why (project_not_opened_yet for one this version has not opened: list tools only read, and never update a project's data; a team project whose server could not be asked, with that reason).",
    schema: schema_for_type::<tasks::ListTasks>,
    read_only: true,
  },
  Spec {
    name: "get_task",
    description: "One task: title, brief, criteria, whether it is a draft or submitted, workflow status, priority, story points (points, set_by person or automation, automation's reason; null for none), tags, module, base_branch (choice: inherit its module's, checked_out or branch, with the branch set; next_run: the branch its next run takes as things stand, null for whatever the checkout has checked out, and its source task, module with that module's id, or checked_out; kept on this computer only, so in a shared project it is this computer's choice), blocked_reason, blocked_by, no_automatic_merge (Don't merge automatically), delivers (what its runs deliver when the task says it; null: as its pipeline says), pipeline, limits, where it stands, what its latest run that passed every step left (result: what it delivers, whether it committed nothing, its completion, its report and the MCP tool calls it was let make outside the repository; null for any other latest run), its runs (get_run gives each one's own base), for a team task closed (how and when a Done or Canceled task was closed: kind person with who, delivery with the module's name, or run with its work; at in ms) and blocked (who set its blocked_reason and when), each null when the team server has no record, as an older server has none, led_in (the Orchestrator conversation its events go to, through the most specific subscription that covers it: its own, its module's, the tasks without a module's or the whole project's, named as `via`; null when none does; get_watches reads the subscriptions), and its link in the app (its page, on its Details; each run links to the page's Summary, which lists the runs).",
    schema: schema_for_type::<tasks::GetTask>,
    read_only: true,
  },
  Spec {
    name: "create_task",
    description: "Saves a new draft task. Limits left out take the project's defaults, as the new task form fills them; null is no limit, and there is no maximum. Any published pipeline may be chosen, with Let automation change agent permissions on or off: every published version is one a person published, allowed, or could not object to (accepted behaviour). Moving an existing task to another pipeline is a person's decision (update_task). Its organization is checked as the task form checks it: blocked_reason marks it Blocked (informational: it keeps no run from starting, but the Orchestrator chat does not start the task) and must not be blank (invalid_organization); blocked_by lists the project's tasks that block it, by key or id, each with its gate (merge, the form's, when left out); a task it does not know is task_not_found and a link that closes a cycle invalid_organization. tags are names matched ignoring case, a new name created as a tag as the form's tag picker does. no_automatic_merge (Don't merge automatically) keeps the Orchestrator from merging the task's runs on its own; a shared project's server does not keep it, so there it is refused (shared_task). delivers says what the task's runs deliver instead of what its pipeline says: changes, report (an investigation that ends in a report), outside (work done through MCP tools outside the repository) or changes_or_report. base_branch is the branch its runs start from and deliver into: inherit (its module's, the default; for a task in no module, whatever the checkout has checked out when a run starts), checked_out (that, whatever its module says) or a branch name; a name only a remote has (release or origin/release) creates its local branch, tracking it, and a name the repository has nowhere is refused as branch_not_found before anything is saved. It is kept on this computer only: in a shared project it sets this computer's choice and is never sent to the server. The task's history records the organization as automation's. story_points (1, 2, 3, 5, 8, 13 or 21) are recorded as automation's; left out, Orchester estimates them when a person turned on Let Orchester estimate story points. In a team project the task goes to its team server: its pipeline must be one of the team's pipelines, sent by its id (a pipeline of this computer's own is refused as team_pipeline_required; the team's Admins create team pipelines), and limits are refused as decision_required. Its workflow_status must be the default (Backlog): a team task is moved on afterwards with update_task (team_status_on_create).",
    schema: schema_for_type::<task_write::CreateTask>,
    read_only: false,
  },
  Spec {
    name: "update_task",
    description: "Changes a task's details or organization; fields left out stay as they are, and a null limit is no limit. Changing a task's pipeline to another pipeline is a person's decision: refused as person_only unless a person turned on Let automation change agent permissions (a task without a pipeline may get any published one). workflow_status done or canceled closes the task, as the task page's and the Tasks list's Mark as done and Cancel task do, whatever its latest run did (failed or canceled runs included); either is refused as stop_run_first while a run of the task has not ended (cancel_run stops it first), and a closed task leaves In progress, Needs you and the queue (its group does not start it). workflow_status backlog or todo reopens it. workflow_status todo on a task whose latest run ended is the Tasks board's Move to Todo: only the status changes, no run starts and nothing is queued (run_task runs it again). Deleting a task is a person's alone, in the app: no tool deletes one. blocked_reason sets the manual Blocked with its reason (not blank: invalid_organization) and null clears it; it is informational, but the Orchestrator chat does not start a task marked Blocked. blocked_by is the whole list of the project's blocking tasks, by key or id, each with its gate (merge, the form's, when left out), and [] clears it; a task it does not know is task_not_found and a link that closes a cycle invalid_organization. tags is the whole list of tag names, matched ignoring case, a new name created as a tag as the form's tag picker does, and [] clears them. no_automatic_merge (Don't merge automatically) true keeps the Orchestrator from merging the task's runs on its own, false lets it again; a shared project's server does not keep it, so changing it there is refused (shared_task) before anything is saved. delivers (changes, report, outside or changes_or_report) says what the task's runs deliver instead of what its pipeline says, and null follows its pipeline again. base_branch sets the branch the task's next runs start from and deliver into: inherit (its module's), checked_out or a branch name; a name only a remote has creates its local branch, tracking it, and one the repository has nowhere is refused as branch_not_found before anything is saved. Runs that started keep the base they recorded. It is kept on this computer only: in a shared project it sets this computer's choice and is never sent to the server. The task's history records the organization changes as automation's. story_points (1, 2, 3, 5, 8, 13 or 21) are recorded as automation's and refused as persons_points when a person set the task's points; after a change of the brief or criteria without them, Orchester estimates them again when a person turned on Let Orchester estimate story points. In a team project the change goes to its team server: a pipeline must be one of the team's pipelines, sent by its id (team_pipeline_required otherwise), and limits are refused as decision_required. Its workflow_status and blocked_reason are the task lifecycle on the server: done and blocked_reason are the claim holder's or an Admin's (not_holder for someone else, not_admin when the change needs an Admin), canceled, backlog and todo are an Admin's only (a closed task is reopened with backlog or todo), and in_progress and in_review follow the task's work (team_status_not_offered); done and canceled close the task and release its claim, refused as stop_run_first while its work runs, and a closed task cannot be claimed or started until an Admin reopens it.",
    schema: schema_for_type::<task_write::UpdateTask>,
    read_only: false,
  },
  Spec {
    name: "get_story_point_calibration",
    description: "How a project's story points tracked reality: per points value (1, 2, 3, 5, 8, 13, 21) the merged tasks that had it and their median tokens, active agent minutes and attempts, and each merged task with its points (who set them) and what it actually took over all its runs. What estimates read as examples. The estimator's own requests are a kind of usage of their own: get_usage names their tokens under `estimates`.",
    schema: schema_for_type::<tasks::GetCalibration>,
    read_only: true,
  },
  Spec {
    name: "run_task",
    description: "Runs a task as the app allows: queues and starts a draft, starts a queued task, or runs a task again after its latest run ended. Refusals name why. A team project's task starts through its team server instead: it is claimed for this computer (claim.acquire) and its work started (work.start) in its module, from the module's branch here; the run takes what this computer filled in (the connection picked for each agent profile, else the only one of its CLI; the program and the switch of each team MCP server; secret values by name), and a pipeline that is not ready here starts nothing and counts no attempt: it is refused as not_ready_here, with the missing parts (each says its fix) in the error's `parts`. A start the server or the runner can't take now waits and goes on by itself (team_start_waiting). A team task runs only with the team pipeline it names; one that is not a team pipeline on this computer is refused as team_pipeline_missing before anything is claimed.",
    schema: schema_for_type::<run_task::RunTask>,
    read_only: false,
  },
  Spec {
    name: "get_run",
    description: "A run: its steps (each with context: how full its latest execution's agent session is, as \
                  {used, window, estimated}: the tokens its latest model response took in, the context window, null \
                  when nobody reported one and no model limit is known, and estimated when the window is a model \
                  limit Orchester assumes rather than one the CLI or provider reported; context is null while the \
                  session's CLI reported nothing: unknown, not 0%), state (for one the runner interrupted after its \
                  own failure, reason \
                  runner_failed with runner_failure: storage_contention when the project database stayed busy, \
                  storage_failure when it was unavailable, other, or null when not recorded), tokens (auto mode's \
                  reviewer included, and its part apart; output_unreported counts its agent invocations whose CLI \
                  reported input but no output yet: their output is unknown, not 0), active time and \
                  what it waits for, its agents' waiting questions with their options, the steps that wait for a \
                  message with what each one's agent wrote last (awaiting.messages: the question it ended its step with, \
                  or its newest message), and the plan waiting for a decision (version, step, its whole text and an \
                  excerpt) among it; and its decided plans, each with its decision, note, when and decided_by (person, \
                  or automation through this server). base is the base the run started from and delivers into, as \
                  it recorded it when it started: its branch (null when it took a detached HEAD's), revision, and \
                  source: task (the task's base branch), module (its module's, with that module's id), checked_out \
                  (whatever the checkout had checked out), or project (a run recorded before tasks and modules had \
                  a base branch); a later change of the task's or module's base branch leaves it alone. merge says \
                  whether its work was merged, as its task's Summary \
                  tells it: merged by orchester (with the recorded merge) or outside (git proves its base branch \
                  holds the run's commits), not_merged, nothing_to_merge, or null for a run that has not succeeded. \
                  worktree is present, removed, not_removed (its removal could not finish and its folder is on disk: \
                  worktree_not_removed gives its path and the reason, null when none was recorded) or none. \
                  base_moved lists Orchester's notices that the run's base branch moved (kind base_moved): the \
                  branch and revision, the tasks and commits that landed, the files both sides changed, and its \
                  state (waits_for_writing_step, waits_until_work_goes_back with waits_for_step when only steps \
                  that never merge the base such as Review or Verify are left, kept_until_run_goes_on, \
                  waits_for_follow_up, delivered with the agent and step that read it, or merged_by_runner with \
                  merged_by_runner.before_step and merge_commit when it overlapped none of the run's changes and \
                  Orchester merged it itself before that step started). delivery is the run's \
                  place in the project's merge queue (null when its pipeline has no Deliver step or it never joined): \
                  lane (the base branch it delivers into; each lane delivers one run at a time, independently of the \
                  others), state queued (place: its place in its lane's line), syncing, rechecking, integrating \
                  (integrate_step, files), waiting_for_approval, blocked (blocked.why: uncommitted_changes with files, \
                  other_branch_checked_out, operation_in_progress, local_integration), failed or needs_you (reason.why: \
                  conflicts with files, check_failed, base_keeps_moving with heads, stopped, later_run), merged \
                  (revision, kind merge_commit or squash, worktree_removed, branch_deleted) or removed; since, \
                  resyncs, approved_at and checked_on (the base the last passed check ran on). A run the merge queue \
                  delivers is not merged by merge_run. merge_attempts lists every attempt to merge the run, newest \
                  first: sequence, at, who (person, automation, merge_queue, or retry with its retry number), via \
                  (task_page, merge_run or merge_queue), outcome (merged with revision and kind, refused, conflicts \
                  with files, not_recorded with the revision Git merged to, git_stopped, left_changed with the files \
                  left changed, interrupted when the app or runner ended mid-merge, stopped, moved when the base or \
                  the run's branch moved), stage (the one it ended at, null when it landed or reached none), cause \
                  (why it did not land, or the merge queue's words; told only as merge_run's policy allows, else \
                  null or the policy's notice) and stages (each stage with passed, refused or not_reached: policy \
                  for automation's attempts, integration_lock, project_database, scratch_folder, checkout_identity, \
                  git_metadata, existing_result, merge, record). checks is the CI of the pull request of the run's \
                  branch, read now through gh (gh pr checks), at most once in 30 s: state (passed, failed, running, \
                  no_checks; or why it was not read: no_pull_request when the last sync stored none, not_linked \
                  without a linked GitHub repository, gh_missing, not_signed_in, wrong_account, missing_scope, \
                  gh_unavailable, rate_limited with resets_at, github_unreachable, github_server_error, not_read; never \
                  passed when unread), read_at, next_read_at (when to read again while a check is queued or running, \
                  else null), counts by state, and runs: each check's name, workflow, state (queued, running, passed, \
                  failed, cancelled, skipped), url and, for a failed GitHub Actions job, log_tail (the last 40 lines of \
                  its failed steps' log, null when gh gave none). approval_mode is the mode its agents' requests are \
                  answered in now (dangerous_auto_approve with because project while its project approves every one at \
                  once; switched_to_manual keeps the run's own choice), approve_mode the project's Approve mode, and \
                  requests how each tool request was answered (answered.kind autopilot, \
                  profile, reviewer, person or dangerous_auto_approve) and why one came to the person (came_to_you). \
                  The run's link is its task's page; the waiting \
                  approvals, questions, messages and plans link to their cards on the task's Conversation. A team task's run \
                  is read from this computer's own records of it, without the task's definition: task is the key the run \
                  recorded when it started (a run that kept none, whose task has no copy here either, is refused as \
                  team_task_unrecorded). team (null for a run that is no team run) says what the team server made of \
                  it: fence (claim_lost, membership_revoked, session_mismatch or superseded once the server took its \
                  authority away; such a run reports nothing more and is stopped here; null otherwise) and done, the \
                  Done its merge asked the server to set on its task (null before the run is merged here): state \
                  pending, sent, or not_sent with reason (the server's refusal code such as not_holder, or canceled \
                  when the task was canceled meanwhile), which a person then settles on the task.",
    schema: schema_for_type::<runs::InRun>,
    read_only: true,
  },
  Spec {
    name: "list_runs",
    description: "Runs, newest first, as each task's Summary lists them: id, number, task (key and title), where it stands (phase; merged also when git proves its work merged outside Orchester), its state and why (with runner_failure, as get_run gives it, for a run the runner interrupted after its own failure), its place in the merge queue (delivery, as get_run gives it: queued, syncing, rechecking, integrating, waiting_for_approval, blocked, failed, needs_you, merged or removed; null when it never joined), what happened, tokens used and its budget, when it started and last changed, and its link in the app (its task's page). At most `limit` runs, the newest: 50 when left out, 200 at most; `more` says whether older ones were left out. With `task`, that task's runs (needs `project`). Without `project`, every project's, each naming its project; unreadable_projects lists the projects that could not be read, with why. A run that cannot be read has no phase, state, tokens or start time; its detail says why. On a team project a team task's runs are listed from this computer's own records of them (the task's key and title are the ones the run recorded when it started); a team task named by key or id that this computer ran nothing of lists none.",
    schema: schema_for_type::<run_list::ListRuns>,
    read_only: true,
  },
  Spec {
    name: "list_questions",
    description: "Agents' questions waiting for an answer, oldest first: id, agent, when it was asked, whether the agent waits in its turn, the questions with their suggested answers, the run (id and number), the task, and the link to its card on the task's Conversation in the app; and, under waiting_for_message, the steps of tasks' latest runs that wait for a message, each with what its agent wrote last (the question it ended its step with, or its newest message), its run, task and link (send_message answers them). With `run`, that run's (needs `project`). Without `project`, every project's, each naming its project; unreadable_projects lists the projects that could not be read, with why. Reading only: answering is a person's, or answer_question's and send_message's. A team task's questions name the task by the key its run recorded when it started.",
    schema: schema_for_type::<questions::ListQuestions>,
    read_only: true,
  },
  Spec {
    name: "get_conversation",
    description: "A task's conversation as its Conversation tab shows it for the task's latest run, oldest first and newest last: what the person and the Orchestrator chat wrote to the agents (with whom to and whether it was delivered), what the agents wrote, the questions they wrote in the conversation (open while their step waits for the answer), their questions with suggested answers (options, state, answers, answered_by: person, automation or orchestrator), their plans (whole text, whether it waits for a decision, the decision), how a decision went on with a run that stopped (run_continued: returned, returned_with_note or restarted, to which step, the note, the attempt; its author the person, automation or the orchestrator), Orchester's notices that the base branch moved (kind base_moved: branch, revision, tasks and commits that landed, overlapping_files, state and delivered_to) and how each step ended; each with its author, step, run, time, `cursor` and the link to its card on the task's Conversation. At most `limit` entries, the newest (50 when left out, 200 at most); `before` (an entry's cursor) reads the entries older than it, and `more` says whether older ones are left. context is how full the agent session of the run's current step (context_step, its key) is, as get_run gives each step's: {used, window, estimated}, or null while its CLI reported nothing or the task has no run. Texts are as stored, secrets redacted; tool calls and approvals have their own tools. On a team project the task is the one its team server reads (its key or server id; team_unavailable while the server cannot be reached), and the conversation is this computer's record of its runs; a team task nothing ran for here has no entries.",
    schema: schema_for_type::<conversation::GetConversation>,
    read_only: true,
  },
  Spec {
    name: "get_plan",
    description: "A plan a gated step of a run proposed, as its card shows it: version, step, agent, when it was proposed, its whole text, whether the run waits on it for a decision, its decision if it was decided (approved, or changes_requested with the note; decided_by and when), the step's versions oldest first (without their text), the run, and the link to its card on the task's Conversation in the app. Left out, `step` is the step whose plan the run waits on (the one decide_plan decides), else the step of the run's latest plan, and `version` the step's latest. Refused as no_plan for a run whose steps proposed none, and as plan_not_found for a step or version without a plan. Reading only: deciding is a person's, or decide_plan's.",
    schema: schema_for_type::<plan_read::GetPlan>,
    read_only: true,
  },
  Spec {
    name: "hold_queue",
    description: "Holds new starts: in the whole project, or, with a task, in the reviewed group it belongs to. Running runs go on; queued tasks wait, a start is refused, until release_queue lets them go on. Answers what holds now. A task of a team project is read from its team server; it is held through its group when this computer has one for it, else refused as not_in_group.",
    schema: schema_for_type::<queue_hold::QueueHold>,
    read_only: false,
  },
  Spec {
    name: "release_queue",
    description: "Lets new starts go on again: in the whole project, or, with a task, in the reviewed group it belongs to. A group's queued tasks then start as its scheduling allows. Answers what holds now. A task of a team project is read from its team server; it is released through its group when this computer has one for it, else refused as not_in_group.",
    schema: schema_for_type::<queue_hold::QueueHold>,
    read_only: false,
  },
  Spec {
    name: "pause_run",
    description: "Pauses a run at its agents' next checkpoint.",
    schema: schema_for_type::<runs::InRun>,
    read_only: false,
  },
  Spec {
    name: "resume_run",
    description: "Resumes a paused run, or continues an interrupted one from its last checkpoint. On a team project a paused run resumes, and an interrupted one continues, under the claim its work started with, and takes no claim of the old sharing stream; a run the team server fenced is not resumed (claim_fenced).",
    schema: schema_for_type::<runs::InRun>,
    read_only: false,
  },
  Spec {
    name: "cancel_run",
    description: "Cancels a run that has not ended.",
    schema: schema_for_type::<runs::InRun>,
    read_only: false,
  },
  Spec {
    name: "send_message",
    description: "Sends a message to the agents of a task's latest run, as the task's Conversation composer sends it in the run's state: to the agent at work (now or after its turn), as the answer to a waiting question or the changes a waiting plan needs, as a follow-up of a run ready for review, as instructions that continue a failed run or one waiting for a decision, resuming a paused or interrupted run, or running an ended task again with it. A run stopped at a limit keeps it until a person raises the limit. Refused when no agent reads it and the task does not run again. On a team project the message goes to the run under the claim its work started with; a message that would run the task again is refused before it is stored (team_run_again_unsupported), because a team task's work starts only through the team server (run_task); one that continues an interrupted run continues it under that claim. A run the team server fenced takes no message in any state, a plan's changes included: it is refused as claim_fenced before anything is stored.",
    schema: schema_for_type::<messages::SendMessage>,
    read_only: false,
  },
  Spec {
    name: "get_approvals",
    description: "A project's approvals: its Approve mode (approve_mode: manual, autopilot or dangerous_auto_approve; in dangerous_auto_approve every tool request of the project's agents is approved at once), whether it allows Autopilot for its agents (allow_autopilot: true in autopilot and dangerous_auto_approve), which connection, model and effort review Manual's requests (reviewer, or why it has none), and each run that has not ended (runs: id, number, task, link) with the approval mode it answers its agents' requests in now (approval_mode: mode autopilot, manual with because profile, project or run, or dangerous_auto_approve with because project while the project's Approve mode approves every request; switched_to_manual: whether the run switched itself to Manual, which applies again once the project leaves Dangerous auto-approve). A step is in Autopilot only when its agent profile says Autopilot (get_profile's permissions.approval_mode), the project allows it and the run was not switched to Manual. The runs listed include a team task's runs, named by the key they recorded when they started.",
    schema: schema_for_type::<approval_settings::GetApprovals>,
    read_only: true,
  },
  Spec {
    name: "set_allow_autopilot",
    description: "The former Allow Autopilot switch, kept for existing clients: false moves the project's Approve mode to manual, so every agent of the project runs as Manual, and is always taken (from dangerous_auto_approve too). true asks for autopilot and is refused as person_only from manual, whatever automation may do: only a person allows Autopilot, in the app (asking for it while Autopilot is allowed already, in autopilot or dangerous_auto_approve, changes nothing). set_approve_mode moves the mode to any of its three values. Answers as get_approvals does.",
    schema: schema_for_type::<approval_settings::SetAllowAutopilot>,
    read_only: false,
  },
  Spec {
    name: "set_approve_mode",
    description: "Moves a project's Approve mode, as Project settings › Runs › Approvals does: manual (every agent runs as Manual, whatever its profile says), autopilot (agents whose profile is set to Autopilot approve their own requests) or dangerous_auto_approve (every tool request of every agent of the project's runs is approved at once: no allowed list, hold rule, reviewer, person or Orchestrator is asked, and each answer is recorded as dangerous_auto_approve). Moving it down (or asking for what it is already) is always taken; moving it up is refused as person_only, whatever automation may do: only a person moves it up, in the app, and chooses dangerous_auto_approve only through its confirmation. Answers as get_approvals does.",
    schema: schema_for_type::<approval_settings::SetApproveMode>,
    read_only: false,
  },
  Spec {
    name: "set_reviewer",
    description: "Chooses the reviewer Manual asks before the person. account names what a language reviewer runs on, a sign-in or an API provider of the person by id or name (list_accounts; cli says whose account a name is and is needed for default), with model and effort; decision_connection names a decision reviewer with model and threshold (0.01–0.99, two decimals, default 0.80). Without either, uses the default language reviewer. Choosing a hosted decision reviewer is always person_only. Threshold edits and switching back to a language reviewer are narrowing. Requires Let automation answer approvals; other reviewer changes require Let automation change agent permissions. Answers as get_approvals does.",
    schema: schema_for_type::<approval_settings::SetReviewer>,
    read_only: false,
  },
  Spec {
    name: "set_run_manual",
    description: "Switches a run to Manual for itself, as its status line does: its agents' requests no longer go through Autopilot, but through the profile's allowed commands, then the reviewer, then the person. A run never goes back to Autopilot. Always taken for a run that has not ended, whatever the person's switches (refused as run_ended otherwise); answers with the run's approval mode and why it is Manual (profile, project or run).",
    schema: schema_for_type::<approval_settings::SetRunManual>,
    read_only: false,
  },
  Spec {
    name: "get_auto_mode",
    description: "Retired with auto mode: changes nothing and answers the error retired. Use get_approvals.",
    schema: schema_for_type::<approval_settings::Retired>,
    read_only: true,
  },
  Spec {
    name: "set_auto_mode",
    description: "Retired with auto mode: changes nothing and answers the error retired. Use set_allow_autopilot and set_reviewer.",
    schema: schema_for_type::<approval_settings::Retired>,
    read_only: true,
  },
  Spec {
    name: "set_run_auto_mode",
    description: "Retired with auto mode: changes nothing and answers the error retired. Use set_run_manual.",
    schema: schema_for_type::<approval_settings::Retired>,
    read_only: true,
  },
  Spec {
    name: "remove_worktree",
    description: "Removes an ended run's worktree as the task page's Remove worktree does: committed work stays on the branches, and only merged branches are deleted. The folder goes with everything in it, build output included; what of the run still runs from it is stopped first when a file is held, and a folder that still cannot be deleted is refused as removal_failed with its path and why, and stays not_removed. Refused while the run works, while a merge or removal of its task runs, when the worktree holds uncommitted changes (a person decides that on the task page), or when it was removed already (a removal whose folder stayed, not_removed, is finished again); refused as run_changed when the run worked again in other worktrees since this read it.",
    schema: schema_for_type::<worktree::RemoveWorktree>,
    read_only: false,
  },
  Spec {
    name: "return_to_draft",
    description: "Returns a queued task whose run never started to Draft and to Backlog, as a card dropped on the Tasks board's Backlog does. A task of a reviewed group goes back only when its group holds it for good (after a task of the group failed, at the group's cap, refused by preflight or the runner, blocked or canceled), as the task list's Return to drafts takes it. A team task has no queue or draft on the team server and is refused as team_no_queue (an Admin moves it back to Backlog with update_task). Refusals name why: not_queued, run_recorded, not_held, shared_server, changed_meanwhile or not_changed.",
    schema: schema_for_type::<return_to_draft::ReturnToDraft>,
    read_only: false,
  },
  Spec {
    name: "continue_run",
    description: "Continues a run stopped at a limit with a choice its task page offers for how it stopped: raise_limits (token budget and time limit), raise_step_budget, resume (after the daily budget), allow_retry, return_to, or accept (a step waiting unverified or unchanged); and resolve_conflicts for a run whose merge into its base branch stopped at conflicts (merge_run's merge_failed): Ask an agent to resolve, which returns its work to a step whose agent may write in the worktree (the implementing one unless step names another) with a note to resolve them, the optional note after it, and the conflicting files as data (Orchester merges the base branch into its worktree first and commits the merge with the agent's work); refused as merge_not_conflicted, step_cannot_write, no_writing_step, integration_running, run_merged, worktree_removed, worktree_in_merge or run_not_ended. Retry, return, resolving and acceptance require Let automation answer approvals and record automation as the decider. Every value is given, null for no limit; nothing raises or lifts a limit by itself. Refusals name why. A team task's run goes on under the claim its work started with: one the team server fenced is refused (claim_fenced), and raising its limits is its Admin's (decision_required).",
    schema: schema_for_type::<continue_run::ContinueRun>,
    read_only: false,
  },
  Spec {
    name: "list_approvals",
    description: "Tool requests of a project's agents waiting for a decision, oldest first: kind, command or tool, arguments, agent, run, task and since when, and `reviewing` while auto mode's reviewer still judges it (it waits for the reviewer, not for a person, until the review ends); and the plans its runs wait on for a decision (decide_plan), with their version, step and an excerpt of their text (get_plan gives a plan's whole text), and the decisions on their earlier versions with decided_by (person, or automation through this server). Each links to its card on its task's Conversation in the app. Without `project`, every project's, each naming its project; unreadable_projects lists the projects that could not be read, with why. `run` needs `project`. A team task's approvals name the task by the key its run recorded when it started.",
    schema: schema_for_type::<approvals::ListApprovals>,
    read_only: true,
  },
  Spec {
    name: "decide_approval",
    description: "Approves a waiting request once, or denies it; a denial may carry a `message`, which the agent gets as the denial's reason (Codex, whose decline carries no words, as the next message of its turn) and which the decision record keeps. Refused unless a person turned on Let automation answer approvals in the app; invalid_arguments for a message with approve_once, an empty one, or one over 2000 characters.",
    schema: schema_for_type::<approvals::DecideApproval>,
    read_only: false,
  },
  Spec {
    name: "decide_plan",
    description: "Decides the plan a gated step of a run waits with, as the plan card on its task page does: approve (Approve plan & continue: the step passes and the steps after it start; takes no note) with the version it is for, as get_run shows it waiting with its whole text, or request_changes with a note (the step's agent resumes with it and proposes a revised plan; a version is optional). The decision is recorded as automation's, and the plan card says so. Both decisions are refused unless a person turned on Let automation answer approvals in the app. Refusals name why: automation_not_allowed, run_not_found, no_plan_waiting, run_paused, plan_already_decided, plan_version_changed, version_required, invalid_version, blank_note, note_not_taken, and the runner's run_not_running, shutting_down, runner_unavailable or runner_reply_lost.",
    schema: schema_for_type::<plans::DecidePlan>,
    read_only: false,
  },
  Spec {
    name: "answer_question",
    description: "Answers an agent's waiting question with suggested answers, as its card in the task conversation does: the labels of the options picked (one, or several where the question allows it) and/or an answer in your own words. Refused unless a person turned on Let automation answer approvals. The answer is recorded as automation's. Also refused for a label the question does not suggest, more than one answer to a single-choice question, or an empty answer.",
    schema: schema_for_type::<questions::AnswerQuestion>,
    read_only: false,
  },
  Spec {
    name: "set_code_index",
    description: "Turns the project's code index on or off for agents that start from now on, as Project settings does. Returns the new state.",
    schema: schema_for_type::<projects::SetCodeIndex>,
    read_only: false,
  },
  Spec {
    name: "update_project_limits",
    description: "Changes a project's limits as Project settings › Runs › Limits does: the default run budget and default time limit in minutes (no maximum; null for no limit), the daily token budget, how many agents work at once (1 to 16, applied at once by the project's runner), how many QA apps its runs may have open at once (1 to 16, from the next start_qa_app) and its share of Services at once (service_slots, 1 to the computer's limit, from the next start). It also sets the computer's Services at once for every project (computer_service_slots, 1 to 32). Limits left out stay as they are; invalid ones are refused with why. Raising or removing the daily budget lets runs it stopped go on, so it is refused as automation_not_allowed unless a person turned on Let automation answer approvals. Returns the limits as saved, null for no limit.",
    schema: schema_for_type::<limits::UpdateProjectLimits>,
    read_only: false,
  },
  Spec {
    name: "duplicate_pipeline",
    description: "Copies a pipeline's latest published version into a new pipeline under a new name, optionally giving agent steps other published profiles, and publishes it as v1. A copy that is not the same as its source (other profiles included) is refused as person_only unless a person turned on Let automation change agent permissions. In a team project the copy goes to the team server at once and is kept here only once the server made it; otherwise it is refused with why and nothing is stored (team_server_refused, team_server_not_reached). Refusals name why.",
    schema: schema_for_type::<pipeline_copy::DuplicatePipeline>,
    read_only: false,
  },
  Spec {
    name: "get_usage",
    description: "Tokens (spent = input - cached input + output, as each CLI reported them) and active agent time of a project's runs, or of every project's, over a range of local days (the last 7 by default, today counted up to now; at most 366, by hour at most 7), as the Usage page reads them: totals, and rows grouped by hour, day, task, pipeline, step, agent (profile; auto mode's reviewer per model), model or provider (the agent connection the tokens were spent on), filtered by task, run, pipeline, step, agent profile, model or provider (an id, or `earlier` for usage no record names a provider for; names in any case); `unmatched` names each filter value nothing in the range matches. The totals' `output_unreported` counts the agent invocations whose CLI reported input but no output yet (Claude Code on some providers reports output only in its final result): their output is unknown, not 0, so `output` and `spent` hold only what the others reported. `link` opens the Usage page with the same query (without `project`, All projects' Usage tab; the page groups by hour or day, so another `group` is by day there); each row's `link` opens it narrowed to the row. `unread_runs` counts the runs left out because their record is damaged; the rest is counted. `estimates`, for one project, is the story-point estimator's own usage in the range (its tokens and how many requests asked a model), outside the runs and their filters; null without `project`.",
    schema: schema_for_type::<usage::GetUsage>,
    read_only: true,
  },
  Spec {
    name: "get_delivery",
    description: "How delivery goes in a project over its last `days` local days (30 when left out, at most 366; today counted up to now), as its Delivery view beside Usage reads it, from what Orchester holds only, in tasks: throughput (tasks merged per day, per week past 31 days, in Orchester or found merged outside it by git, with their story points and how many had none); cycle time (first agent start to merge) and lead time (creation to merge) of each task merged in the range with their median, quartiles and 85th percentile in hours, and its attempts; where agent time goes (active hours per pipeline step, executions, how many were returned by a failure route or a person, the return percent, and attempts per delivered task); cumulative flow (tasks in each workflow status at the end of each day, Canceled left out); and forecasts of when the queued tasks and each open module (only `module`'s, by id or name, beside the queued tasks', when given) are likely delivered: a Monte Carlo over the tasks merged on the project's recent days, giving the 50% and 85% dates, the remaining tasks and the history it rests on (days and tasks merged); not_enough_history instead of dates when the history is too short, nothing_left when no work remains. No money estimates. `link` opens the Delivery view. On a team project nothing is computed from this computer's rows of its tasks, which are partial: the call is refused as team_project_unsupported, after a module named is read from its team server (module_not_found when it has none).",
    schema: schema_for_type::<delivery::GetDelivery>,
    read_only: true,
  },
  Spec {
    name: "get_daily_report",
    description: "A project's report of one local day (today when `day` is left out, up to now), the one its Orchestrator conversation got for that day when it was delivered (delivered: true), else read now from what Orchester holds: the tasks merged that day (key, title, run, when); the tasks' latest runs as they stand, ready to merge, stopped (failed, at a limit, waiting for a decision or interrupted, with `end` telling how and the step) or at work; what waits for the person (tool requests, questions, messages, plans, the Orchestrator's proposals); what the Orchestrator did that day (way: direct, itself, or applied by the person); the day's tokens (spent) and active agent time by task and by account, as get_usage reads them, against the daily budget (null: no limit); and the queued tasks in order. Every item has its app `link`. merge_queue lists the runs in the project's merge queue now and those merged, failed or taken out of it since that day began, each with its run, task and delivery state as get_run gives it (queued, syncing, rechecking, integrating, waiting_for_approval, blocked, failed, needs_you, merged, removed).",
    schema: schema_for_type::<daily_report::GetDailyReport>,
    read_only: true,
  },
  Spec {
    name: "list_pipelines",
    description: "A project's pipelines, which tasks run with; each is always a published version (saving one publishes the next): name, version, steps, and its link in the app (its editor). Without `project`, every project's, each naming its project; unreadable_projects lists the projects that could not be read, with why.",
    schema: schema_for_type::<across::InProjects>,
    read_only: true,
  },
  Spec {
    name: "list_modules",
    description: "A project's modules (epics, milestones or sprints that group its tasks), oldest first: name, description, start and target dates (YYYY-MM-DD, local days), state (open or closed), base_branch (the branch its tasks that inherit theirs start from and deliver into; null for whatever the checkout has checked out when a run starts; kept on this computer only), how many tasks it has, its progress (scope and done in its unit, tasks until tasks have story points; Canceled tasks are out of scope) and its link in the app (its page). Shared projects have no modules.",
    schema: schema_for_type::<modules::ListModules>,
    read_only: true,
  },
  Spec {
    name: "get_module",
    description: "One module, as its page shows it: its details (base_branch among them, as list_modules gives it), progress, its tasks (key, title, workflow status, link) and its burndown by local day from its start (or the day it got its first task) through today or its target date, at most 366 days (a longer module gives its first year while today is in it, else the year up to today): scope and remaining work at the end of each day (null for a day still to come), the ideal line from the scope at the start to nothing at the target date (null without one), and the work added to and removed from the module that day, in the unit it names. Done is the task's Done: merging its run sets it, in Orchester or found merged outside it by git, as does a person. `forecast` is when its remaining work is likely delivered, as get_delivery forecasts it (50% and 85% dates, or not_enough_history). `led_in` is the Orchestrator conversation that leads it, as its Led in shows it (its own subscription, else the whole project's, as `via`), or null when none does; get_watches reads the subscriptions. `own_branch` (null for a module without one) gives a local module's own branch, its acceptance pipeline, Deliver when accepted, how many acceptances it had and where the latest stands (none, under_way, passed, not_passed), its last delivery, and what it waits for once its every task is done (`waits_for`: delivered, accept, accepting, deliver, or person_… when what comes next is the person's).",
    schema: schema_for_type::<modules::GetModule>,
    read_only: true,
  },
  Spec {
    name: "create_module",
    description: "Makes a new open module in a local project, with a name no other module has, an optional description, optional start and target dates (YYYY-MM-DD; the target not before the start) and an optional base_branch: the branch its tasks that inherit theirs start from and deliver into, checked_out (the default: whatever the checkout has checked out when a run starts) or a branch name; a name only a remote has (release or origin/release) creates its local branch, tracking it, and a name the repository has nowhere is refused as branch_not_found before the module is made. The base branch is kept on this computer only and never sent to a server. Put tasks in it with create_task's or update_task's module. Refused as shared_task in a shared project. `own_branch: true` gives a local module a branch of its own, `module/<slug>`, made from its base branch when its first task starts: its tasks' runs start from it and merge into it without the person's working tree, and it reaches its base branch by deliver_module; `acceptance_pipeline` and `deliver_when_accepted` say how (start_acceptance).",
    schema: schema_for_type::<modules::CreateModule>,
    read_only: false,
  },
  Spec {
    name: "update_module",
    description: "Changes a module's name, description, dates (null for none), state (open or closed) or base_branch (checked_out or a branch name, as create_module takes it; the next runs of its tasks that inherit theirs take it, runs that started keep theirs; kept on this computer only and never sent to a server); fields left out stay as they are. Its tasks and the history of what moved in and out stay. `own_branch`, `acceptance_pipeline` (null for none) and `deliver_when_accepted` change a local module's own branch and how it is delivered, as create_module sets them; taking the branch away leaves it in the repository.",
    schema: schema_for_type::<modules::UpdateModule>,
    read_only: false,
  },
  Spec {
    name: "deliver_module",
    description: "Delivers a module that has a branch of its own: merges that branch, at its tip, into the module's base branch with a merge commit (the branch the checkout has checked out when the module names none), as one whole, apart from any run's merge. A local module: refused while a task of it has a run that has not ended (runs_at_work), when its branch holds nothing its base lacks (nothing_to_deliver), for a module without a branch of its own (no_own_branch), and as the merge refuses, with everything left as it was: delivery_conflicts (the base moved and the merge conflicts: bring the base into the module's branch through a task, or resolve it in the app), uncommitted_in_the_way (the person's checkout has changes the merge would overwrite), integration_running. A team module: its delivery is requested from its team server as merge_run of one of its runs requests it, under the module's claim, and an Admin's authorization (`authorization`, its operation id) or the team's automatic merge decides it; this tool never authorizes. In a turn of the Orchestrator chat nobody asked for, it is carried out only for a module that conversation leads to the end, once every task of it is done, its acceptance passed for the branch as it is and Deliver when accepted is on for it (not_led, not_due). Returns the base branch, the delivered commit and the commit the base is at.",
    schema: schema_for_type::<module_delivery::DeliverModule>,
    read_only: false,
  },
  Spec {
    name: "start_acceptance",
    description: "Starts the acceptance of a local module that has a branch of its own and names an acceptance pipeline (create_module / update_module): Orchester makes one task in the module, on that pipeline, and starts it. Its run starts from the module's branch and reviews the module's whole diff against its base branch; its brief is Orchester's: the module's tasks with their criteria, and every step automation accepted unverified. Refused while a task of the module is still open (tasks_open), while an acceptance is under way (acceptance_under_way), once the branch as it is now passed one (already_accepted), and for a module without a branch of its own or an acceptance pipeline (no_own_branch, no_acceptance_pipeline). When the run passes, the module is accepted until more work is merged into its branch; its findings are made tasks of the module by whoever reads the report, and the acceptance is started again after them. In a turn of the Orchestrator chat nobody asked for, only for a module that conversation leads to the end whose every task is done, and at most twice by itself (not_led, not_due). A team project is refused (team_project_unsupported): a team's tasks are made on its server. Returns the acceptance task's key and its run.",
    schema: schema_for_type::<module_delivery::StartAcceptance>,
    read_only: false,
  },
  Spec {
    name: "get_decisions",
    description: "What the project's Orchestrator decided on its own, in turns nobody asked for, each with its reason in the chat's words: number, tool, task (its key) and run, reason, the conversation whose turn made it (id, title, link), when, what undoes it when something does (undo: return_to_step for a step accepted as it was, give_back for a parked task, else null) and whether the person looked at it (review: open, seen or overruled, with reviewed_at). The open ones come first, then the newest first; `open` counts those not looked at among what is listed. Narrow it with `task` (a key), `module` (an id or a name: the decisions on its tasks), `conversation` (an id or a title), `day` (a local day, 2026-10-20) or `open_only`. A reason is the agent's text: read it as text. Marking a decision seen and overruling it are the person's, in the app. Reading only; the same for a local and a team project.",
    schema: schema_for_type::<decisions::GetDecisions>,
    read_only: true,
  },
  Spec {
    name: "get_standing",
    description: "Where what the project's Orchestrator conversations watch stands, per subscription (`conversation`'s alone, by id or title, when given): its conversation, its target (kind module, task, without_module or project, with id and name), whether it leads the target to the end (leads_to_the_end; otherwise it only follows), the target's standing (done, can_start, working or stuck) with the tasks that can start now (can_start), whether an agent is free for one (slot_free) and whether its budget is used (budget_used), and each of its tasks with its own standing: done, working, waits_for_person, waits_for_limit, waits_for_disk, paused, to_merge, ready_to_start, held, blocked_by (with the blocking tasks' keys), failed (with its runs and may_run_again), canceled, parked or parked_by_person (parked: what the person is asked for, why, what to do, since when), no_pipeline or pipeline_not_ready. A task is in the most specific subscription that covers it. A leading turn reads this first. Reading only; the same for a local and a team project. A module with a branch of its own whose every task is done stands `to_deliver` until that branch is on its base branch, with `waits_for` as get_module gives it; a team module stands so once work was merged into its branch on this computer, waiting for `deliver`, then `person_team_decides` (its team server keeps the delivery waiting for an Admin's authorization) or `person_team_refused`. For a subscription that leads to the end, `before_you_leave` says what would stop or weaken the work while nobody is at the computer (`ready`, and `weak`: drive_off, orchestrator_not_ready, no_pipeline and no_deliver_step with their tasks, checkout_has_changes, no_daily_budget, may_sleep, stops_with_window, each with its words), and what the app cannot check from here (`not_checked`); it warns and refuses nothing.",
    schema: schema_for_type::<standing::GetStanding>,
    read_only: true,
  },
  Spec {
    name: "get_watches",
    description: "What a project's Orchestrator conversations lead and what waits there now, as /watching shows it: per conversation (`conversation`'s alone, by id or title, when given; else every one that leads something) its id, title and link, how many modules and tasks it leads (and whether the tasks without a module or the whole project), the one-line total of what waits for the person, and its subscriptions in the order they were kept: target (kind module, task, without_module or project; id, name, link), since (ms), order (source own, project_default or none, with its text), mode (follow, or lead_to_the_end with its `ceilings`: runs_of_task, returns_of_run, raises_of_run, raise_percent, and budget_tokens with spent_tokens when it has a budget), the tasks it leads that have something going on (running, waits with what for, queued with its place, stopped_at_limit, ready_to_merge; each with its run, words and link, and `again` once a task led to the end was run more than once) and the rest counted. Read-only: subscribing, stopping, taking over, the mode and the ceilings stay the person's commands in the conversation, and no tool changes them. get_module and get_task name the leading conversation (`led_in`). In a team project the tasks are the team's: one that ran on this computer is listed as its runs recorded it, and one that never ran here as the team server's last reading of the watched modules holds it; a subscription whose target the server could not be asked about says so (`unreadable`).",
    schema: schema_for_type::<watches::GetWatches>,
    read_only: true,
  },
];
