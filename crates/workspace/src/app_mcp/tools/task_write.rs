//! `create_task` and `update_task`: a task saved as the new task form and the task page save it. The
//! details are read with the form's rules ([`TaskForm`]: each limit optional with no maximum, the
//! project's default limits filled in as the form fills them), and saved through the
//! project's [`TaskWriter`], so a shared project's task goes to its server.

use orchester_core::{
  id::{IdempotencyKey, ModuleId},
  store::SettingsStore,
  task::{
    Task, TaskDetails,
    organization::{ChangeActor, TaskOrganization},
  },
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  modules::find_module,
  tasks::find_task,
  wire::{self, Rank, Workflow},
};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  budget::whole_minutes,
  messages::Message,
  tasks::{
    conflict::{Edit, EditSaved, save_edit},
    data::{PipelineChoice, SaveTarget, published_pipelines},
    form::TaskForm,
    outside::Outside,
    writer::{TaskWriter, WriteError},
  },
};

/// How whole tokens are grouped in the form's text; the tools give plain numbers.
const GROUP_SEPARATOR: &str = ",";

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct CreateTask {
  /// The project's id, or its name.
  pub project: String,
  pub title: String,
  #[serde(default)]
  pub brief: Option<String>,
  /// Acceptance criteria, one per entry; blank ones are left out.
  #[serde(default)]
  pub criteria: Vec<String>,
  /// A published pipeline's id or name.
  #[serde(default)]
  pub pipeline: Option<String>,
  /// The token budget of each run, with no maximum; `null` for no budget, the project's default
  /// when left out.
  #[serde(default, deserialize_with = "wire::limit")]
  pub budget_tokens: Option<Option<u64>>,
  /// Minutes of active agent time each run may take; `null` for no time limit, the project's
  /// default when left out.
  #[serde(default, deserialize_with = "wire::limit")]
  pub time_limit_minutes: Option<Option<u64>>,
  #[serde(default)]
  pub workflow_status: Option<Workflow>,
  #[serde(default)]
  pub priority: Option<Rank>,
  /// The module to put the task in, by its id or name (list_modules).
  #[serde(default)]
  pub module: Option<String>,
}

/// The fields to change; those left out stay as they are.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct UpdateTask {
  /// The project's id, or its name.
  pub project: String,
  /// The task's key, e.g. `KON-12`, or its id.
  pub key: String,
  #[serde(default)]
  pub title: Option<String>,
  #[serde(default)]
  pub brief: Option<String>,
  #[serde(default)]
  pub criteria: Option<Vec<String>>,
  #[serde(default)]
  pub pipeline: Option<String>,
  /// The token budget of each run; `null` for no budget.
  #[serde(default, deserialize_with = "wire::limit")]
  pub budget_tokens: Option<Option<u64>>,
  /// Minutes of active agent time each run may take; `null` for no time limit.
  #[serde(default, deserialize_with = "wire::limit")]
  pub time_limit_minutes: Option<Option<u64>>,
  #[serde(default)]
  pub workflow_status: Option<Workflow>,
  #[serde(default)]
  pub priority: Option<Rank>,
  /// The module to move the task to, by its id or name; `null` takes it out of its module.
  #[serde(default, deserialize_with = "super::modules::nullable")]
  pub module: Option<Option<String>>,
}

/// The details as the form reads them from its fields' text.
struct Fields<'a> {
  title: &'a str,
  brief: &'a str,
  criteria: &'a [String],
  pipeline: Option<&'a PipelineChoice>,
  budget: String,
  minutes: String,
}

impl Fields<'_> {
  fn details(&self) -> Result<TaskDetails, ToolError> {
    let form = TaskForm {
      title: self.title,
      brief: self.brief,
      criteria: self.criteria,
      pipeline: self.pipeline.map(|choice| choice.id),
      budget: &self.budget,
      time_limit: &self.minutes,
      separator: GROUP_SEPARATOR,
    };
    form.details().map_err(|errors| {
      let reasons: Vec<Message> = [errors.title, errors.budget, errors.time_limit]
        .into_iter()
        .flatten()
        .collect();
      ToolError::refused("invalid_task", reasons)
    })
  }
}

pub async fn create_task(services: &Services, args: CreateTask) -> Result<Value, ToolError> {
  let services = services.clone();
  let task = blocking(move || create(&services, &args)).await?;
  Ok(json!({ "task": saved(&task) }))
}

pub async fn update_task(services: &Services, args: UpdateTask) -> Result<Value, ToolError> {
  let services = services.clone();
  let task = blocking(move || update(&services, &args)).await?;
  Ok(json!({ "task": saved(&task) }))
}

fn create(services: &Services, args: &CreateTask) -> Result<Task, ToolError> {
  let open = services.project(&args.project)?;
  let root = open.repo_root;
  let db = ProjectDb::open(&root)?;
  let limits = db.project_limits()?;
  let published = published_pipelines(&db)?;
  let module = args
    .module
    .as_deref()
    .map(|named| find_module(&db, named).map(|module| module.id()))
    .transpose()?;
  drop(db);
  let fields = Fields {
    title: &args.title,
    brief: args.brief.as_deref().unwrap_or_default(),
    criteria: &args.criteria,
    pipeline: args
      .pipeline
      .as_deref()
      .map(|named| pipeline_named(&published, named))
      .transpose()?,
    budget: wire::limit_text(
      args
        .budget_tokens
        .unwrap_or(limits.default_run_budget().map(TokenBudget::get)),
    ),
    minutes: wire::limit_text(
      args
        .time_limit_minutes
        .unwrap_or(limits.default_time_limit().map(whole_minutes)),
    ),
  };
  let details = fields.details()?;
  let base = TaskOrganization::default();
  let edited = organized(base.clone(), args.workflow_status, args.priority);
  let mut writer = TaskWriter::open(&root, &services.servers).map_err(write_error)?;
  let target = SaveTarget::New(IdempotencyKey::generate());
  let task = writer
    .save_organized(target, details, (&base, &edited), Timestamp::now())
    .map_err(write_error)?
    .saved()
    .ok_or_else(changed)?;
  let task = moved(&mut writer, task, module)?;
  services.changes.record(open.project.id(), Outside::Tasks);
  Ok(task)
}

/// `task` put in `module`, or in none, when it is not there already.
fn moved(writer: &mut TaskWriter, task: Task, module: Option<ModuleId>) -> Result<Task, ToolError> {
  if task.module() == module {
    return Ok(task);
  }
  writer
    .set_task_module(task.id(), module, ChangeActor::Person)
    .map_err(write_error)?;
  Ok(task.with_module(module))
}

fn update(services: &Services, args: &UpdateTask) -> Result<Task, ToolError> {
  let open = services.project(&args.project)?;
  let root = open.repo_root;
  let db = ProjectDb::open(&root)?;
  let published = published_pipelines(&db)?;
  let task = find_task(&db, &args.key)?;
  let module = match &args.module {
    Some(Some(named)) => Some(Some(find_module(&db, named)?.id())),
    Some(None) => Some(None),
    None => None,
  };
  drop(db);
  let current = task.details();
  let pipeline = match args.pipeline.as_deref() {
    Some(named) => Some(pipeline_named(&published, named)?),
    None => current
      .pipeline
      .and_then(|id| published.iter().find(|choice| choice.id == id)),
  };
  // Another pipeline runs the task with other steps, profiles and approvals: a person decides that. A task
  // without one may get any published pipeline, as a new task may, and its own pipeline's newer versions
  // passed the publishing rule already.
  if let (Some(chosen), Some(before)) = (pipeline, current.pipeline)
    && chosen.id != before
  {
    super::setup::widening::allowed(services, vec![Message::new("appMcp.widens.taskPipeline")])?;
  }
  let fields = Fields {
    title: args.title.as_deref().unwrap_or(current.title.as_str()),
    brief: args.brief.as_deref().unwrap_or(&current.brief),
    criteria: args.criteria.as_deref().unwrap_or(&current.acceptance_criteria),
    pipeline,
    budget: wire::limit_text(
      args
        .budget_tokens
        .unwrap_or(current.limits.budget().map(TokenBudget::get)),
    ),
    minutes: wire::limit_text(
      args
        .time_limit_minutes
        .unwrap_or(current.limits.max_duration().map(whole_minutes)),
    ),
  };
  let mut details = fields.details()?;
  // A pipeline the project no longer publishes stays chosen until the caller chooses another.
  if args.pipeline.is_none() && pipeline.is_none() {
    details.pipeline = current.pipeline;
  }
  let base = task.organization().clone();
  let edited = organized(base.clone(), args.workflow_status, args.priority);
  let mut writer = TaskWriter::open(&root, &services.servers).map_err(write_error)?;
  let edit = Edit {
    task: task.id(),
    read: task.revision(),
    base: current,
    details,
    organization: (&base, &edited),
  };
  let saved = match save_edit(&mut writer, edit, Timestamp::now()).map_err(write_error)? {
    EditSaved::Saved { task, .. } => task,
    EditSaved::Conflict(_) => return Err(changed()),
  };
  let saved = match module {
    Some(module) => moved(&mut writer, saved, module)?,
    None => saved,
  };
  services.changes.record(open.project.id(), Outside::Tasks);
  Ok(saved)
}

/// `base` with the workflow status and priority given.
fn organized(base: TaskOrganization, status: Option<Workflow>, priority: Option<Rank>) -> TaskOrganization {
  let base = match status {
    Some(status) => base.with_status(status.into()),
    None => base,
  };
  match priority {
    Some(priority) => base.with_priority(priority.into()),
    None => base,
  }
}

/// The published pipeline `named` by its id or, ignoring case, its name.
pub(super) fn pipeline_named<'a>(
  published: &'a [PipelineChoice],
  named: &str,
) -> Result<&'a PipelineChoice, ToolError> {
  let lowered = named.to_lowercase();
  published
    .iter()
    .find(|choice| choice.id.to_string() == named || choice.name.to_lowercase() == lowered)
    .ok_or_else(|| ToolError::not_found(Missing::Pipeline, named))
}

/// What the tools say about a saved task.
fn saved(task: &Task) -> Value {
  json!({
    "key": task.key().to_string(),
    "id": task.id().to_string(),
    "submission": wire::submission(task.status()),
    "workflow_status": Workflow::from(task.organization().status()),
    "priority": Rank::from(task.organization().priority()),
    "module": task.module().map(|module| module.to_string()),
  })
}

/// The task changed while it was being saved, in a way that conflicts: nothing was saved.
fn changed() -> ToolError {
  ToolError::refused(
    "changed_meanwhile",
    vec![Message::new("tasks.conflict.changedMeanwhile")],
  )
}

pub fn write_error(error: WriteError) -> ToolError {
  match error {
    WriteError::Store(error) => error.into(),
    WriteError::Sharing(_) => ToolError::refused("shared_server", vec![error.message()]),
  }
}
