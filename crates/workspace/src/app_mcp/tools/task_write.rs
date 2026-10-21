//! `create_task` and `update_task`: a task saved as the new task form and the task page save it. The
//! details are read with the form's rules ([`TaskForm`]: each limit optional with no maximum, the
//! project's default limits filled in as the form fills them), the organization as the form gives it
//! ([`super::task_organization`]), and saved through the project's [`TaskWriter`], so a shared
//! project's task goes to its server. The task's history records the organization changes as
//! automation's: they come from a client of the app's MCP server or the Orchestrator chat.

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
  task_organization::{Blocker, Changes},
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
    conflict::{Edit, EditSaved, save_edit_by},
    data::{PipelineChoice, SaveTarget, published_pipelines},
    form::TaskForm,
    outside::Outside,
    writer::{TaskWriter, WriteError},
  },
};

/// How whole tokens are grouped in the form's text; the tools give plain numbers.
const GROUP_SEPARATOR: &str = ",";

/// Who the task's history names for the organization these tools save.
const ACTOR: ChangeActor = ChangeActor::Orchester;

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
  /// Marks the task Blocked with this reason, which must not be blank. Informational: it keeps no
  /// run from starting, but the Orchestrator chat does not start the task.
  #[serde(default)]
  pub blocked_reason: Option<String>,
  /// The tasks that block this one: it cannot run until each is finished.
  #[serde(default)]
  pub blocked_by: Vec<Blocker>,
  /// Tag names, matched ignoring case; a name the project has no tag of is created as a tag.
  #[serde(default)]
  pub tags: Vec<String>,
  /// Don't merge automatically: `true` keeps the Orchestrator from merging the task's runs on its own, even while
  /// the person lets it merge finished runs; left out, `false`.
  #[serde(default)]
  pub no_automatic_merge: bool,
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
  /// Marks the task Blocked with this reason, which must not be blank; `null` clears it.
  /// Informational: it keeps no run from starting, but the Orchestrator chat does not start the task.
  #[serde(default, deserialize_with = "wire::clearable")]
  pub blocked_reason: Option<Option<String>>,
  /// The whole list of tasks that block this one, which it cannot run before each is finished;
  /// `[]` clears it.
  #[serde(default)]
  pub blocked_by: Option<Vec<Blocker>>,
  /// The whole list of tag names, matched ignoring case; a name the project has no tag of is
  /// created as a tag. `[]` clears them.
  #[serde(default)]
  pub tags: Option<Vec<String>>,
  /// Don't merge automatically: `true` keeps the Orchestrator from merging the task's runs on its own, even while
  /// the person lets it merge finished runs; `false` lets it again.
  #[serde(default)]
  pub no_automatic_merge: Option<bool>,
}

impl CreateTask {
  fn changes(&self) -> Changes<'_> {
    Changes {
      status: self.workflow_status,
      priority: self.priority,
      blocked_reason: self.blocked_reason.as_deref().map(Some),
      blocked_by: Some(&self.blocked_by),
      tags: Some(&self.tags),
      no_automatic_merge: Some(self.no_automatic_merge),
    }
  }
}

impl UpdateTask {
  fn changes(&self) -> Changes<'_> {
    Changes {
      status: self.workflow_status,
      priority: self.priority,
      blocked_reason: self.blocked_reason.as_ref().map(Option::as_deref),
      blocked_by: self.blocked_by.as_deref(),
      tags: self.tags.as_deref(),
      no_automatic_merge: self.no_automatic_merge,
    }
  }
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
  let base = TaskOrganization::default();
  let changes = args.changes();
  let edited = changes.apply(&db, &base)?;
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
  let mut writer = TaskWriter::open(&root, &services.servers).map_err(write_error)?;
  // A move a shared project refuses is refused before the task is made, so a retry makes no second task.
  if module.is_some() {
    writer.ensure_modules().map_err(write_error)?;
  }
  let edited = changes.tagged(&mut writer, edited)?;
  let target = SaveTarget::New(IdempotencyKey::generate());
  let task = writer
    .save_organized_by(target, details, (&base, &edited), (ACTOR, Timestamp::now()))
    .map_err(write_error)?
    .saved()
    .ok_or_else(changed)?;
  let task = moved(&mut writer, task, module)?;
  services.changes.record(open.project.id(), Outside::Tasks);
  Ok(task)
}

/// `task` put in `module`, or in none, when it is not there already; the move is recorded as automation's.
fn moved(writer: &mut TaskWriter, task: Task, module: Option<ModuleId>) -> Result<Task, ToolError> {
  if task.module() == module {
    return Ok(task);
  }
  writer.set_task_module(task.id(), module, ACTOR).map_err(write_error)?;
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
  let base = task.organization().clone();
  let changes = args.changes();
  let edited = changes.apply(&db, &base)?;
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
  let mut writer = TaskWriter::open(&root, &services.servers).map_err(write_error)?;
  // A move a shared project refuses is refused before the task's details are saved.
  if matches!(module, Some(Some(_))) {
    writer.ensure_modules().map_err(write_error)?;
  }
  let edited = changes.tagged(&mut writer, edited)?;
  let edit = Edit {
    task: task.id(),
    read: task.revision(),
    base: current,
    details,
    organization: (&base, &edited),
  };
  let saved = match save_edit_by(&mut writer, edit, (ACTOR, Timestamp::now())).map_err(write_error)? {
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
    "blocked_reason": task.organization().blocked_reason(),
    "no_automatic_merge": task.organization().no_automatic_merge(),
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
