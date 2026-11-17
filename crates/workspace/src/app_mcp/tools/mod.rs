//! The tools of Orchester's MCP server: their names, what they do, their input schemas (strict: an
//! unknown or mistyped argument is refused) and how a call reaches them. Every tool goes through the
//! services the screens use; none of them holds a rule of its own.

mod across;
mod approval_settings;
mod approvals;
mod base_branch;
mod base_moved;
mod ci_wire;
mod continue_run;
mod conversation;
mod daily_report;
mod decisions;
mod delivery;
mod limits;
mod merge_attempts;
mod merge_queue;
mod message_waits;
mod messages;
mod module_delivery;
mod modules;
mod orchestrated;
mod pipeline_copy;
mod plan_read;
mod plans;
mod projects;
mod questions;
mod queue_hold;
mod readiness_json;
mod return_to_draft;
mod review;
mod run_list;
mod run_task;
mod runs;
pub mod setup;
mod specs;
mod standing;
mod task_organization;
mod task_write;
mod tasks;
mod team;
mod usage;
mod watches;
mod wire;
mod worktree;

use orchester_core::id::ChatId;
use rmcp::model::{JsonObject, Tool, ToolAnnotations};
use serde::de::DeserializeOwned;
use serde_json::Value;

pub(crate) use self::{
  continue_run::{Choice as ContinueChoice, ContinueRun, Offered as ContinueOffered, offered as continue_offered},
  messages::{SendTiming, plan_person_only, refusal as outgoing_refusal, sent_as},
  modules::find_module,
  pipeline_copy::duplicate_pipeline_in_chat,
  questions::{check_answer, waiting_request},
  queue_hold::not_in_group,
  tasks::find_task,
};
pub use self::{
  run_task::{Proposal, propose_run, run_proposed},
  runs::find_run,
};
use self::{
  runs::Control,
  specs::{SPECS, Spec},
};
use super::{error::ToolError, services::Services};

/// The team task `key` names (its key or its server id) in project `project`, as its team server reads it now;
/// `None` in a local project and for a local task of a team project. Not found when the server has no such task.
pub(crate) async fn team_task(
  services: &Services,
  project: &str,
  key: &str,
) -> Result<Option<orchester_api::v2::TaskRead>, ToolError> {
  team::read_task(services, project, key).await
}
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
    "get_run" => runs::get_run(services, parse(arguments)?, translate).await,
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

/// The tools that answer what agents wait for, the project's approvals, the pull requests and the daily report.
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
    "get_approvals" => approval_settings::get_approvals(services, parse(arguments)?).await,
    "set_allow_autopilot" => approval_settings::set_allow_autopilot(services, parse(arguments)?).await,
    "set_approve_mode" => approval_settings::set_approve_mode(services, parse(arguments)?).await,
    "set_reviewer" => approval_settings::set_reviewer(services, parse(arguments)?).await,
    "set_run_manual" => approval_settings::set_run_manual(services, parse(arguments)?).await,
    "get_auto_mode" => Err(approval_settings::retired(name, "get_approvals")),
    "set_auto_mode" => Err(approval_settings::retired(name, "set_allow_autopilot and set_reviewer")),
    "set_run_auto_mode" => Err(approval_settings::retired(name, "set_run_manual")),
    other => dispatch_planning(services, translate, other, arguments).await,
  }
}

/// The tools of the project's modules, what its Orchestrator conversations watch and decide, and its reports.
async fn dispatch_planning(
  services: &Services,
  translate: &Translate,
  name: &str,
  arguments: Value,
) -> Result<Value, ToolError> {
  match name {
    "list_modules" => modules::list_modules(services, parse(arguments)?).await,
    "get_module" => modules::get_module(services, parse(arguments)?, translate).await,
    "create_module" => modules::create_module(services, parse(arguments)?).await,
    "update_module" => modules::update_module(services, parse(arguments)?).await,
    "deliver_module" => module_delivery::deliver_module(services, parse(arguments)?, translate).await,
    "start_acceptance" => module_delivery::start_acceptance(services, parse(arguments)?).await,
    "get_watches" => watches::get_watches(services, parse(arguments)?, translate).await,
    "get_decisions" => decisions::get_decisions(services, parse(arguments)?).await,
    "get_standing" => standing::get_standing(services, parse(arguments)?, translate).await,
    "get_daily_report" => daily_report::get_daily_report(services, parse(arguments)?).await,
    "get_delivery" => delivery::get_delivery(services, parse(arguments)?).await,
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
