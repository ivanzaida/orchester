//! The tools of Orchester's MCP server: their names, what they do, their input schemas (strict: an
//! unknown or mistyped argument is refused) and how a call reaches them. Every tool goes through the
//! services the screens use; none of them holds a rule of its own.

mod across;
mod approvals;
mod auto_mode;
mod base_moved;
mod continue_run;
mod conversation;
mod daily_report;
mod limits;
mod message_waits;
mod messages;
mod orchestrated;
mod pipeline_copy;
mod plan_read;
mod plans;
mod projects;
mod pull_request_reads;
mod pull_request_wire;
mod pull_requests;
mod questions;
mod queue_hold;
mod return_to_draft;
mod run_list;
mod run_task;
mod runs;
pub mod setup;
mod task_organization;
mod task_write;
mod tasks;
mod usage;
mod wire;
mod worktree;

use std::sync::Arc;

use orchester_core::id::ChatId;
use rmcp::{
  handler::server::common::schema_for_type,
  model::{JsonObject, Tool, ToolAnnotations},
};
use serde::de::DeserializeOwned;
use serde_json::Value;

pub(super) use self::pull_request_reads::RecentReads;
use self::runs::Control;
pub(crate) use self::{
  continue_run::{Choice as ContinueChoice, ContinueRun, Offered as ContinueOffered, offered as continue_offered},
  messages::{SendTiming, plan_person_only, refusal as outgoing_refusal, sent_as},
  pipeline_copy::duplicate_pipeline_draft,
  questions::{check_answer, waiting_request},
  queue_hold::not_in_group,
  tasks::find_task,
};
pub use self::{
  run_task::{Proposal, propose_run, run_proposed},
  runs::find_run,
};
use super::{error::ToolError, services::Services};
use crate::messages::Message;

/// A message in the words the app shows it, for results that carry the app's own explanations:
/// the app words [`Message`]s of this crate's locale files ([`crate::i18n`]).
pub struct Translate(Box<dyn Fn(&Message) -> String + Send + Sync>);

impl Translate {
  pub fn new(words: impl Fn(&Message) -> String + Send + Sync + 'static) -> Self {
    Self(Box::new(words))
  }

  pub fn text(&self, message: &Message) -> String {
    (self.0)(message)
  }
}

/// Who sends an answer or a message to a run's agents: a client of the app's MCP server, or the project's
/// Orchestrator chat of a conversation, whose answers the task's conversation names as the chat's, with the
/// conversation it wrote them in and how they went out: sent by the chat itself, or applied by the person.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SentBy {
  Client,
  Orchestrator(ChatId),
  /// A native, person-only acceptance of a proposed answer of the chat's conversation.
  Person(ChatId),
}

impl SentBy {
  const fn answerer(self) -> orchester_core::run::Answerer {
    match self {
      Self::Person(_) => orchester_core::run::Answerer::Person,
      Self::Client | Self::Orchestrator(_) => orchester_core::run::Answerer::Automation,
    }
  }

  /// The Orchestrator chat's conversation that wrote it; `None` for a client's.
  const fn chat(self) -> Option<ChatId> {
    match self {
      Self::Client => None,
      Self::Orchestrator(chat) | Self::Person(chat) => Some(chat),
    }
  }
}

/// One tool as clients list it.
struct Spec {
  name: &'static str,
  description: &'static str,
  schema: fn() -> Arc<JsonObject>,
  /// It reads and changes nothing.
  read_only: bool,
}

const SPECS: &[Spec] = &[
  Spec {
    name: "list_projects",
    description: "The projects of the workspace, most recently opened first: id, name, folder, whether each uses the code index, its limits (token budgets, default time limit, agents working at once), and its link in the app (its Overview). Links are the app's route paths; the person opens one with a click.",
    schema: schema_for_type::<projects::ListProjects>,
    read_only: true,
  },
  Spec {
    name: "list_tasks",
    description: "A project's tasks, as its task list finds them: key, title, workflow status, priority, story points (points, set_by person or automation, automation's reason; null for none), tags, blocked_reason (the manual Blocked's reason, or null), held_by_blocking_tasks (whether unfinished blocking tasks keep it from running), pipeline, where the task stands, and its link in the app (its page). Without `project`, every project's, each naming its project, as stored: git is asked which runs were merged outside Orchester only for a project named; unreadable_projects lists the projects that could not be read, with why (project_not_opened_yet for one this version has not opened: list tools only read, and never update a project's data).",
    schema: schema_for_type::<tasks::ListTasks>,
    read_only: true,
  },
  Spec {
    name: "get_task",
    description: "One task: title, brief, criteria, whether it is a draft or submitted, workflow status, priority, story points (points, set_by person or automation, automation's reason; null for none), tags, blocked_reason, blocked_by, pipeline, limits, where it stands, its runs, and its link in the app (its page, on its Details; each run links to the page's Summary, which lists the runs).",
    schema: schema_for_type::<tasks::GetTask>,
    read_only: true,
  },
  Spec {
    name: "create_task",
    description: "Saves a new draft task. Limits left out take the project's defaults, as the new task form fills them; null is no limit, and there is no maximum. Any published pipeline may be chosen, with Let automation change agent permissions on or off: every published version is one a person published, allowed, or could not object to (accepted behaviour). Moving an existing task to another pipeline is a person's decision (update_task). Its organization is checked as the task form checks it: blocked_reason marks it Blocked (informational: it keeps no run from starting, but the Orchestrator chat does not start the task) and must not be blank (invalid_organization); blocked_by lists the project's tasks that block it, by key or id, each with its gate (merge, the form's, when left out); a task it does not know is task_not_found and a link that closes a cycle invalid_organization. tags are names matched ignoring case, a new name created as a tag as the form's tag picker does. The task's history records the organization as automation's. story_points (1, 2, 3, 5, 8, 13 or 21) are recorded as automation's; left out, Orchester estimates them when a person turned on Let Orchester estimate story points.",
    schema: schema_for_type::<task_write::CreateTask>,
    read_only: false,
  },
  Spec {
    name: "update_task",
    description: "Changes a task's details or organization; fields left out stay as they are, and a null limit is no limit. Changing a task's pipeline to another pipeline is a person's decision: refused as person_only unless a person turned on Let automation change agent permissions (a task without a pipeline may get any published one). blocked_reason sets the manual Blocked with its reason (not blank: invalid_organization) and null clears it; it is informational, but the Orchestrator chat does not start a task marked Blocked. blocked_by is the whole list of the project's blocking tasks, by key or id, each with its gate (merge, the form's, when left out), and [] clears it; a task it does not know is task_not_found and a link that closes a cycle invalid_organization. tags is the whole list of tag names, matched ignoring case, a new name created as a tag as the form's tag picker does, and [] clears them. The task's history records the organization changes as automation's. story_points (1, 2, 3, 5, 8, 13 or 21) are recorded as automation's and refused as persons_points when a person set the task's points; after a change of the brief or criteria without them, Orchester estimates them again when a person turned on Let Orchester estimate story points.",
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
    description: "Runs a task as the app allows: queues and starts a draft, starts a queued task, or runs a task again after its latest run ended. Refusals name why.",
    schema: schema_for_type::<run_task::RunTask>,
    read_only: false,
  },
  Spec {
    name: "get_run",
    description: "A run: its steps, state, tokens (auto mode's reviewer included, and its part apart), active time and \
                  what it waits for, its agents' waiting questions with their options, the steps that wait for a \
                  message with what each one's agent wrote last (awaiting.messages: the question it ended its step with, \
                  or its newest message), and the plan waiting for a decision (version, step, its whole text and an \
                  excerpt) among it; and its decided plans, each with its decision, note, when and decided_by (person, \
                  or automation through this server). merge says whether its work was merged, as its task's Summary \
                  tells it: merged by orchester (with the recorded merge) or outside (git proves its base branch \
                  holds the run's commits), not_merged, nothing_to_merge, or null for a run that has not succeeded. \
                  base_moved lists Orchester's notices that the run's base branch moved (kind base_moved): the \
                  branch and revision, the tasks and commits that landed, the files both sides changed, and its \
                  state (waits_for_writing_step, waits_until_work_goes_back with waits_for_step when only steps \
                  that never merge the base such as Review or Verify are left, kept_until_run_goes_on, \
                  waits_for_follow_up, or delivered with the agent and step that read it). The run's link is its \
                  task's page; the waiting approvals, \
                  questions, messages and plans link to their cards on the task's Conversation.",
    schema: schema_for_type::<runs::InRun>,
    read_only: true,
  },
  Spec {
    name: "list_runs",
    description: "Runs, newest first, as each task's Summary lists them: id, number, task (key and title), where it stands (phase; merged also when git proves its work merged outside Orchester), its state and why, what happened, tokens used and its budget, when it started and last changed, and its link in the app (its task's page). At most `limit` runs, the newest: 50 when left out, 200 at most; `more` says whether older ones were left out. With `task`, that task's runs (needs `project`). Without `project`, every project's, each naming its project; unreadable_projects lists the projects that could not be read, with why. A run that cannot be read has no phase, state, tokens or start time; its detail says why.",
    schema: schema_for_type::<run_list::ListRuns>,
    read_only: true,
  },
  Spec {
    name: "list_questions",
    description: "Agents' questions waiting for an answer, oldest first: id, agent, when it was asked, whether the agent waits in its turn, the questions with their suggested answers, the run (id and number), the task, and the link to its card on the task's Conversation in the app; and, under waiting_for_message, the steps of tasks' latest runs that wait for a message, each with what its agent wrote last (the question it ended its step with, or its newest message), its run, task and link (send_message answers them). With `run`, that run's (needs `project`). Without `project`, every project's, each naming its project; unreadable_projects lists the projects that could not be read, with why. Reading only: answering is a person's, or answer_question's and send_message's.",
    schema: schema_for_type::<questions::ListQuestions>,
    read_only: true,
  },
  Spec {
    name: "get_conversation",
    description: "A task's conversation as its Conversation tab shows it for the task's latest run, oldest first and newest last: what the person and the Orchestrator chat wrote to the agents (with whom to and whether it was delivered), what the agents wrote, the questions they wrote in the conversation (open while their step waits for the answer), their questions with suggested answers (options, state, answers, answered_by: person, automation or orchestrator), their plans (whole text, whether it waits for a decision, the decision), how a decision went on with a run that stopped (run_continued: returned, returned_with_note or restarted, to which step, the note, the attempt; its author the person, automation or the orchestrator), Orchester's notices that the base branch moved (kind base_moved: branch, revision, tasks and commits that landed, overlapping_files, state and delivered_to) and how each step ended; each with its author, step, run, time, `cursor` and the link to its card on the task's Conversation. At most `limit` entries, the newest (50 when left out, 200 at most); `before` (an entry's cursor) reads the entries older than it, and `more` says whether older ones are left. Texts are as stored, secrets redacted; tool calls and approvals have their own tools.",
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
    description: "Holds new starts: in the whole project, or, with a task, in the reviewed group it belongs to. Running runs go on; queued tasks wait, a start is refused, until release_queue lets them go on. Answers what holds now.",
    schema: schema_for_type::<queue_hold::QueueHold>,
    read_only: false,
  },
  Spec {
    name: "release_queue",
    description: "Lets new starts go on again: in the whole project, or, with a task, in the reviewed group it belongs to. A group's queued tasks then start as its scheduling allows. Answers what holds now.",
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
    description: "Resumes a paused run, or continues an interrupted one from its last checkpoint.",
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
    description: "Sends a message to the agents of a task's latest run, as the task's Conversation composer sends it in the run's state: to the agent at work (now or after its turn), as the answer to a waiting question or the changes a waiting plan needs, as a follow-up of a run ready for review, as instructions that continue a failed run or one waiting for a decision, resuming a paused or interrupted run, or running an ended task again with it. A run stopped at a limit keeps it until a person raises the limit. Refused when no agent reads it and the task does not run again.",
    schema: schema_for_type::<messages::SendMessage>,
    read_only: false,
  },
  Spec {
    name: "set_run_auto_mode",
    description: "Turns a run's auto mode on or off: a reviewer model answers the requests the rules leave to the person, approving only those that clearly serve the task. Refused unless a person turned on Let automation answer approvals in the app.",
    schema: schema_for_type::<auto_mode::SetRunAutoMode>,
    read_only: false,
  },
  Spec {
    name: "get_auto_mode",
    description: "A project's auto mode settings: whether new runs use it, and which connection, model and effort review requests (or why auto mode has no reviewer).",
    schema: schema_for_type::<auto_mode::GetAutoMode>,
    read_only: true,
  },
  Spec {
    name: "set_auto_mode",
    description: "Changes a project's auto mode settings: whether new runs use it, and the reviewer (an agent connection of the project, a model of it and an effort it takes; without a connection, the default reviewer). Refused unless a person turned on Let automation answer approvals in the app.",
    schema: schema_for_type::<auto_mode::SetAutoMode>,
    read_only: false,
  },
  Spec {
    name: "remove_worktree",
    description: "Removes an ended run's worktree as the task page's Remove worktree does: committed work stays on the branches, and only merged branches are deleted. Refused while the run works, while a merge or removal of its task runs, when the worktree holds uncommitted changes (a person decides that on the task page), or when it was removed already; refused as run_changed when the run worked again in other worktrees since this read it.",
    schema: schema_for_type::<worktree::RemoveWorktree>,
    read_only: false,
  },
  Spec {
    name: "return_to_draft",
    description: "Returns a queued task whose run never started to Draft, as the task list's Return to drafts does: only a task its group holds for good (after a task of the group failed, at the group's cap, refused by preflight or the runner, blocked or canceled). Refusals name why.",
    schema: schema_for_type::<return_to_draft::ReturnToDraft>,
    read_only: false,
  },
  Spec {
    name: "continue_run",
    description: "Continues a run stopped at a limit with a choice its task page offers for how it stopped: raise_limits (token budget and time limit), raise_step_budget, resume (after the daily budget), allow_retry, return_to, or accept (a step waiting unverified or unchanged); and resolve_conflicts for a run whose merge into its base branch stopped at conflicts (merge_run's merge_failed): Ask an agent to resolve, which returns its work to a step whose agent may write in the worktree (the implementing one unless step names another) with a note to merge the base branch in and resolve them, the optional note after it, and the conflicting files as data; refused as merge_not_conflicted, step_cannot_write, no_writing_step, integration_running, run_merged, worktree_removed, worktree_in_merge or run_not_ended. Retry, return, resolving and acceptance require Let automation answer approvals and record automation as the decider. Every value is given, null for no limit; nothing raises or lifts a limit by itself. Refusals name why.",
    schema: schema_for_type::<continue_run::ContinueRun>,
    read_only: false,
  },
  Spec {
    name: "list_approvals",
    description: "Tool requests of a project's agents waiting for a decision, oldest first: kind, command or tool, arguments, agent, run, task and since when; and the plans its runs wait on for a decision (decide_plan), with their version, step and an excerpt of their text (get_plan gives a plan's whole text), and the decisions on their earlier versions with decided_by (person, or automation through this server). Each links to its card on its task's Conversation in the app. Without `project`, every project's, each naming its project; unreadable_projects lists the projects that could not be read, with why. `run` needs `project`.",
    schema: schema_for_type::<approvals::ListApprovals>,
    read_only: true,
  },
  Spec {
    name: "decide_approval",
    description: "Approves a waiting request once, or denies it. Refused unless a person turned on Let automation answer approvals in the app.",
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
    description: "Changes a project's limits as Project settings › Limits does: the default run budget and default time limit in minutes (no maximum; null for no limit), the daily token budget, how many agents work at once (1 to 16, applied at once by the project's runner) and how many QA apps its runs may have open at once (1 to 16, from the next start_qa_app). Limits left out stay as they are; invalid ones are refused with why. Returns the limits as saved, null for no limit.",
    schema: schema_for_type::<limits::UpdateProjectLimits>,
    read_only: false,
  },
  Spec {
    name: "duplicate_pipeline",
    description: "Copies a pipeline's latest published version into a new pipeline under a new name, optionally giving agent steps other published profiles, and publishes it as v1. A copy that is not the same as its source (other profiles included) is refused as person_only unless a person turned on Let automation change agent permissions. Refusals name why.",
    schema: schema_for_type::<pipeline_copy::DuplicatePipeline>,
    read_only: false,
  },
  Spec {
    name: "list_pull_requests",
    description: "Pull requests of a project's linked GitHub repository as the Pull requests page lists them, or of every project: number, title, state (open, draft, merged, closed), review, GitHub checks on the head, branches, changes, when it was merged or closed, whether it comes from a fork, the task and run whose branch it is (agent_working while that run runs), its app link and its GitHub url. They are what the last successful sync read (open ones and the 50 most recently updated closed ones). Each repository says how its latest sync went (sync: synced, rate_limited, github_unreachable, paused_other_account, gh_unavailableâ€¦), when the list was read (synced_at), when the latest sync ran (attempted_at) and, during a rate limit, until when syncs wait (paused_until); unreadable_projects lists the projects that could not be read, with why. refresh syncs one project (needs project) first through gh, unless a sync ran in the last 30 s or a rate limit holds: then the stored list answers. A tab narrows them as the page's tabs do; search finds a number, title words, a branch or a task key.",
    schema: schema_for_type::<pull_requests::ListPullRequests>,
    read_only: true,
  },
  Spec {
    name: "get_pull_request",
    description: "One pull request of a project's linked GitHub repository, as list_pull_requests gives it, with how the repository's latest sync went. One the last sync did not read (an older closed one) is refused as pull_request_not_synced; with refresh it is read from GitHub on its own, and a refresh of the same number within 30 s answers what that read found. A read GitHub refuses says why: github_rate_limited (until paused_until), github_unreachable, or pull_request_not_read; pull_request_not_found when GitHub has no such pull request.",
    schema: schema_for_type::<pull_requests::GetPullRequest>,
    read_only: true,
  },
  Spec {
    name: "get_usage",
    description: "Tokens (spent = input - cached input + output, as each CLI reported them) and active agent time of a project's runs, or of every project's, over a range of local days (the last 7 by default, today counted up to now; at most 366, by hour at most 7), as the Usage page reads them: totals, and rows grouped by hour, day, task, pipeline, step, agent (profile; auto mode's reviewer per model), model or account, filtered by task, run, pipeline, step, agent profile, model or account (an id, or `none` for usage on no account; names in any case); `unmatched` names each filter value nothing in the range matches. `link` opens the Usage page with the same query (without `project`, All projects' Usage tab; the page groups by hour or day, so another `group` is by day there); each row's `link` opens it narrowed to the row. `unread_runs` counts the runs left out because their record is damaged; the rest is counted. `estimates`, for one project, is the story-point estimator's own usage in the range (its tokens and how many requests asked a model), outside the runs and their filters; null without `project`.",
    schema: schema_for_type::<usage::GetUsage>,
    read_only: true,
  },
  Spec {
    name: "get_daily_report",
    description: "A project's report of one local day (today when `day` is left out, up to now), the one its Orchestrator conversation got for that day when it was delivered (delivered: true), else read now from what Orchester holds: the tasks merged that day (key, title, run, when); the tasks' latest runs as they stand, ready to merge, stopped (failed, at a limit, waiting for a decision or interrupted, with `end` telling how and the step) or at work; what waits for the person (tool requests, questions, messages, plans, the Orchestrator's proposals); what the Orchestrator did that day (way: direct, itself, or applied by the person); the day's tokens (spent) and active agent time by task and by account, as get_usage reads them, against the daily budget (null: no limit); and the queued tasks in order. Every item has its app `link`.",
    schema: schema_for_type::<daily_report::GetDailyReport>,
    read_only: true,
  },
  Spec {
    name: "list_pipelines",
    description: "A project's pipelines that have a published version, which tasks run with: name, version, steps, and its link in the app (its editor). Without `project`, every project's, each naming its project; unreadable_projects lists the projects that could not be read, with why.",
    schema: schema_for_type::<across::InProjects>,
    read_only: true,
  },
];

/// Every tool, as `tools/list` answers.
pub fn listed() -> Vec<Tool> {
  SPECS.iter().map(tool).collect()
}

/// `spec` as clients list it.
fn tool(spec: &Spec) -> Tool {
  Tool::new(spec.name, spec.description, (spec.schema)())
    .with_annotations(ToolAnnotations::new().read_only(spec.read_only).open_world(false))
}

/// Calls tool `name` with `arguments`; `None` when there is no such tool.
pub async fn call(
  services: &Services,
  translate: &Translate,
  name: &str,
  arguments: JsonObject,
) -> Option<Result<Value, ToolError>> {
  if !SPECS.iter().any(|spec| spec.name == name) {
    return None;
  }
  Some(dispatch(services, translate, name, Value::Object(arguments)).await)
}

async fn dispatch(
  services: &Services,
  translate: &Translate,
  name: &str,
  arguments: Value,
) -> Result<Value, ToolError> {
  match name {
    "list_projects" => projects::list_projects(services, parse(arguments)?).await,
    "list_tasks" => tasks::list_tasks(services, parse(arguments)?, translate).await,
    "get_task" => tasks::get_task(services, parse(arguments)?, translate).await,
    "create_task" => task_write::create_task(services, parse(arguments)?).await,
    "update_task" => task_write::update_task(services, parse(arguments)?).await,
    "run_task" => run_task::run_task(services, parse(arguments)?).await,
    "get_run" => runs::get_run(services, parse(arguments)?).await,
    "list_runs" => run_list::list_runs(services, parse(arguments)?, translate).await,
    "pause_run" => runs::control_run(services, parse(arguments)?, Control::Pause).await,
    "resume_run" => runs::control_run(services, parse(arguments)?, Control::Resume).await,
    "cancel_run" => runs::control_run(services, parse(arguments)?, Control::Cancel).await,
    "return_to_draft" => return_to_draft::return_to_draft(services, parse(arguments)?).await,
    "continue_run" => continue_run::continue_run(services, parse(arguments)?).await,
    "send_message" => messages::send_message(services, parse(arguments)?, (translate, SentBy::Client, None)).await,
    "remove_worktree" => worktree::remove_worktree(services, parse(arguments)?, translate).await,
    "list_pipelines" => projects::list_pipelines(services, parse(arguments)?, translate).await,
    "get_usage" => usage::get_usage(services, parse(arguments)?).await,
    "set_code_index" => projects::set_code_index(services, parse(arguments)?).await,
    "update_project_limits" => limits::update_project_limits(services, parse(arguments)?).await,
    "duplicate_pipeline" => pipeline_copy::duplicate_pipeline(services, parse(arguments)?, translate).await,
    "hold_queue" => queue_hold::set_queue_hold(services, parse(arguments)?, true).await,
    "release_queue" => queue_hold::set_queue_hold(services, parse(arguments)?, false).await,
    other => dispatch_answers(services, translate, other, arguments).await,
  }
}

/// The tools that answer what agents wait for, auto mode's settings, the pull requests and the daily report.
async fn dispatch_answers(
  services: &Services,
  translate: &Translate,
  name: &str,
  arguments: Value,
) -> Result<Value, ToolError> {
  match name {
    "list_approvals" => approvals::list_approvals(services, parse(arguments)?, translate).await,
    "list_questions" => questions::list_questions(services, parse(arguments)?, translate).await,
    "get_conversation" => conversation::get_conversation(services, parse(arguments)?).await,
    "get_plan" => plan_read::get_plan(services, parse(arguments)?).await,
    "decide_approval" => approvals::decide_approval(services, parse(arguments)?).await,
    "answer_question" => questions::answer_question(services, parse(arguments)?, SentBy::Client).await,
    "decide_plan" => plans::decide_plan(services, parse(arguments)?).await,
    "set_run_auto_mode" => auto_mode::set_run_auto_mode(services, parse(arguments)?).await,
    "get_auto_mode" => auto_mode::get_auto_mode(services, parse(arguments)?).await,
    "set_auto_mode" => auto_mode::set_auto_mode(services, parse(arguments)?).await,
    "list_pull_requests" => pull_requests::list_pull_requests(services, parse(arguments)?, translate).await,
    "get_pull_request" => pull_requests::get_pull_request(services, parse(arguments)?).await,
    "get_daily_report" => daily_report::get_daily_report(services, parse(arguments)?).await,
    "get_story_point_calibration" => tasks::get_calibration(services, parse(arguments)?).await,
    other => Err(ToolError::InvalidArguments(format!("no tool is named {other}"))),
  }
}

/// Calls `answer_question` or `send_message` (`name`) with `arguments` for the project's Orchestrator chat of
/// conversation `chat`, decided `by`: named the chat's in the task's conversation, with where it came from and how it
/// went out (sent by the chat as automation, or applied by the person), and refused where it would decide a waiting
/// plan.
pub async fn answer_for_orchestrator(
  services: &Services,
  translate: &Translate,
  name: &str,
  arguments: JsonObject,
  (chat, by, expected): (ChatId, orchester_core::run::Answerer, Option<&str>),
) -> Result<Value, ToolError> {
  let arguments = Value::Object(arguments);
  let by = match by {
    orchester_core::run::Answerer::Person => SentBy::Person(chat),
    orchester_core::run::Answerer::Automation => SentBy::Orchestrator(chat),
  };
  match name {
    "answer_question" => questions::answer_question(services, parse(arguments)?, by).await,
    "send_message" => messages::send_message(services, parse(arguments)?, (translate, by, expected)).await,
    other => Err(ToolError::InvalidArguments(format!("{other} answers no agent"))),
  }
}

/// `continue_run` with `arguments` for the Orchestrator chat, decided by `by`: a person who applied the chat's
/// proposal decides as themselves ([`continue_run::continue_as`]).
pub(crate) async fn continue_for_orchestrator(
  services: &Services,
  arguments: JsonObject,
  by: orchester_core::run::Answerer,
) -> Result<Value, ToolError> {
  continue_run::continue_as(services, parse(Value::Object(arguments))?, by).await
}

/// The arguments as tool `T` takes them; a mismatch is the call's error.
fn parse<T: DeserializeOwned>(arguments: Value) -> Result<T, ToolError> {
  serde_json::from_value(arguments).map_err(|error| ToolError::InvalidArguments(error.to_string()))
}
