//! The tools of Orchester's MCP server: their names, what they do, their input schemas (strict: an
//! unknown or mistyped argument is refused) and how a call reaches them. Every tool goes through the
//! services the screens use; none of them holds a rule of its own.

mod approvals;
mod auto_mode;
mod limits;
mod pipeline_copy;
mod projects;
mod run_task;
mod runs;
mod task_write;
mod tasks;
mod wire;
mod worktree;

use std::sync::Arc;

use lurq::app::i18n::I18n;
use rmcp::{
  handler::server::common::schema_for_type,
  model::{JsonObject, Tool, ToolAnnotations},
};
use serde::de::DeserializeOwned;
use serde_json::Value;

use self::runs::Control;
use super::{error::ToolError, services::Services};
use crate::messages::Message;

/// A message in the words the app shows it, for results that carry the app's own explanations.
pub struct Translate(I18n);

impl Translate {
  pub fn new(i18n: I18n) -> Self {
    Self(i18n)
  }

  pub fn text(&self, message: &Message) -> String {
    let args = message.args.iter().map(|(name, value)| (*name, value.as_str()));
    self.0.t_args(message.key, args).to_string()
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
    description: "The projects of the workspace, most recently opened first: id, name, folder, whether each uses the code index, and its limits (token budgets, default time limit, agents working at once).",
    schema: schema_for_type::<projects::ListProjects>,
    read_only: true,
  },
  Spec {
    name: "list_tasks",
    description: "A project's tasks, as its task list finds them: key, title, workflow status, priority, tags, pipeline and where the task stands.",
    schema: schema_for_type::<tasks::ListTasks>,
    read_only: true,
  },
  Spec {
    name: "get_task",
    description: "One task: title, brief, criteria, whether it is a draft or submitted, workflow status, priority, tags, pipeline, limits, where it stands and runs.",
    schema: schema_for_type::<tasks::GetTask>,
    read_only: true,
  },
  Spec {
    name: "create_task",
    description: "Saves a new draft task. Limits left out take the project's defaults, as the new task form fills them.",
    schema: schema_for_type::<task_write::CreateTask>,
    read_only: false,
  },
  Spec {
    name: "update_task",
    description: "Changes a task's details or organization; fields left out stay as they are.",
    schema: schema_for_type::<task_write::UpdateTask>,
    read_only: false,
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
                  what it waits for.",
    schema: schema_for_type::<runs::InRun>,
    read_only: true,
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
    description: "Removes an ended run's worktree as the task page's Remove worktree does: committed work stays on the branches, and only merged branches are deleted. Refused while the run works, while a merge or removal of its task runs, when the worktree holds uncommitted changes (a person decides that on the task page), or when it was removed already.",
    schema: schema_for_type::<worktree::RemoveWorktree>,
    read_only: false,
  },
  Spec {
    name: "list_approvals",
    description: "Tool requests of a project's agents waiting for a decision, oldest first: kind, command or tool, arguments, agent, run, task and since when.",
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
    name: "set_code_index",
    description: "Turns the project's code index on or off for agents that start from now on, as Project settings does. Returns the new state.",
    schema: schema_for_type::<projects::SetCodeIndex>,
    read_only: false,
  },
  Spec {
    name: "update_project_limits",
    description: "Changes a project's limits as Project settings › Limits does: maximum, default and daily token budgets, the default time limit in minutes and how many agents work at once (1 to 16, applied at once by the project's runner). Limits left out stay as they are; invalid ones are refused with why. Returns the limits as saved.",
    schema: schema_for_type::<limits::UpdateProjectLimits>,
    read_only: false,
  },
  Spec {
    name: "duplicate_pipeline",
    description: "Copies a pipeline's latest published version into a new pipeline under a new name, optionally giving agent steps other published profiles, and publishes it as v1. Refusals name why.",
    schema: schema_for_type::<pipeline_copy::DuplicatePipeline>,
    read_only: false,
  },
  Spec {
    name: "list_pipelines",
    description: "A project's pipelines that have a published version, which tasks run with: name, version, steps.",
    schema: schema_for_type::<projects::InProject>,
    read_only: true,
  },
];

/// Every tool, as `tools/list` answers.
pub fn listed() -> Vec<Tool> {
  SPECS
    .iter()
    .map(|spec| {
      Tool::new(spec.name, spec.description, (spec.schema)())
        .with_annotations(ToolAnnotations::new().read_only(spec.read_only).open_world(false))
    })
    .collect()
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
    "pause_run" => runs::control_run(services, parse(arguments)?, Control::Pause).await,
    "resume_run" => runs::control_run(services, parse(arguments)?, Control::Resume).await,
    "cancel_run" => runs::control_run(services, parse(arguments)?, Control::Cancel).await,
    "list_approvals" => approvals::list_approvals(services, parse(arguments)?).await,
    "remove_worktree" => worktree::remove_worktree(services, parse(arguments)?, translate).await,
    "set_run_auto_mode" => auto_mode::set_run_auto_mode(services, parse(arguments)?).await,
    "get_auto_mode" => auto_mode::get_auto_mode(services, parse(arguments)?).await,
    "set_auto_mode" => auto_mode::set_auto_mode(services, parse(arguments)?).await,
    "decide_approval" => approvals::decide_approval(services, parse(arguments)?).await,
    "list_pipelines" => projects::list_pipelines(services, parse(arguments)?, translate).await,
    "set_code_index" => projects::set_code_index(services, parse(arguments)?).await,
    "update_project_limits" => limits::update_project_limits(services, parse(arguments)?).await,
    "duplicate_pipeline" => pipeline_copy::duplicate_pipeline(services, parse(arguments)?, translate).await,
    other => Err(ToolError::InvalidArguments(format!("no tool is named {other}"))),
  }
}

/// The arguments as tool `T` takes them; a mismatch is the call's error.
fn parse<T: DeserializeOwned>(arguments: Value) -> Result<T, ToolError> {
  serde_json::from_value(arguments).map_err(|error| ToolError::InvalidArguments(error.to_string()))
}
