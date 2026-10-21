//! `list_modules`, `get_module`, `create_module` and `update_module`: a project's modules as the Modules list and a
//! module's page show them (progress, tasks, the burndown by local day), and modules made and changed as the module
//! form does, through the project's [`TaskWriter`]. A shared project has no modules: its tasks change on the server.

use orchester_core::{
  id::ProjectId,
  module::{Module, ModuleDetails, ModuleState},
  store::TaskStore,
};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Deserializer};
use serde_json::{Value, json};

use super::{
  task_write::write_error,
  wire::{self, Workflow, task_link},
};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  links::{Link, TaskPage},
  modules::{
    burndown::Burndown,
    data::{ModulePage, ModuleSummary, Progress, summaries},
    form::ModuleForm,
  },
  tasks::{outside::Outside, writer::TaskWriter},
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct ListModules {
  /// The project's id, or its name.
  pub project: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct GetModule {
  /// The project's id, or its name.
  pub project: String,
  /// The module's id, or its name.
  pub module: String,
}

/// Whether a module is still worked on.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(crate = "rmcp::schemars")]
pub enum State {
  Open,
  Closed,
}

impl From<State> for ModuleState {
  fn from(state: State) -> Self {
    match state {
      State::Open => Self::Open,
      State::Closed => Self::Closed,
    }
  }
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct CreateModule {
  /// The project's id, or its name.
  pub project: String,
  pub name: String,
  #[serde(default)]
  pub description: Option<String>,
  /// The first day the module counts from, `YYYY-MM-DD`.
  #[serde(default)]
  pub start_date: Option<String>,
  /// The day its work should be done by, `YYYY-MM-DD`.
  #[serde(default)]
  pub target_date: Option<String>,
}

/// The fields to change; those left out stay as they are, and a null date is no date.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct UpdateModule {
  /// The project's id, or its name.
  pub project: String,
  /// The module's id, or its name.
  pub module: String,
  #[serde(default)]
  pub name: Option<String>,
  #[serde(default)]
  pub description: Option<String>,
  #[serde(default, deserialize_with = "nullable")]
  pub start_date: Option<Option<String>>,
  #[serde(default, deserialize_with = "nullable")]
  pub target_date: Option<Option<String>>,
  #[serde(default)]
  pub state: Option<State>,
}

/// An argument that may be given as `null` (`Some(None)`); one left out is `None`.
pub fn nullable<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Option<String>>, D::Error> {
  Option::<String>::deserialize(deserializer).map(Some)
}

pub async fn list_modules(services: &Services, args: ListModules) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let db = ProjectDb::open(&open.repo_root)?;
    let listed = summaries(&db.modules()?, &db.tasks()?);
    let project = open.project.id();
    Ok(json!({
      "modules": listed.iter().map(|summary| summary_json(project, summary)).collect::<Vec<_>>(),
      "link": Link::Modules(project).path(),
    }))
  })
  .await
}

pub async fn get_module(services: &Services, args: GetModule) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let db = ProjectDb::open(&open.repo_root)?;
    let module = find_module(&db, &args.module)?;
    let page = ModulePage::read(&db, module.id(), orchester_core::time::Timestamp::now())?
      .ok_or_else(|| ToolError::not_found(Missing::Module, &args.module))?;
    Ok(page_json(open.project.id(), &page))
  })
  .await
}

pub async fn create_module(services: &Services, args: CreateModule) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let form = ModuleForm {
      name: &args.name,
      description: args.description.as_deref().unwrap_or_default(),
      start: args.start_date.as_deref().unwrap_or_default(),
      target: args.target_date.as_deref().unwrap_or_default(),
      state: ModuleState::Open,
    };
    let details = form_details(&form)?;
    let mut writer = TaskWriter::open(&open.repo_root, &services.servers).map_err(write_error)?;
    let module = writer.create_module(details).map_err(write_error)?;
    services.changes.record(open.project.id(), Outside::Tasks);
    Ok(json!({ "module": module_json(open.project.id(), &module) }))
  })
  .await
}

pub async fn update_module(services: &Services, args: UpdateModule) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let module = find_module(&ProjectDb::open(&open.repo_root)?, &args.module)?;
    let current = module.details();
    let day = |given: &Option<Option<String>>, stored: Option<chrono::NaiveDate>| match given {
      Some(value) => value.clone().unwrap_or_default(),
      None => stored.map(|day| day.to_string()).unwrap_or_default(),
    };
    let (start, target) = (
      day(&args.start_date, current.start()),
      day(&args.target_date, current.target()),
    );
    let form = ModuleForm {
      name: args.name.as_deref().unwrap_or(current.name().as_str()),
      description: args.description.as_deref().unwrap_or(current.description()),
      start: &start,
      target: &target,
      state: args.state.map_or(current.state(), Into::into),
    };
    let details = form_details(&form)?;
    let mut writer = TaskWriter::open(&open.repo_root, &services.servers).map_err(write_error)?;
    let module = writer
      .update_module(module.id(), current, details)
      .map_err(write_error)?;
    services.changes.record(open.project.id(), Outside::Tasks);
    Ok(json!({ "module": module_json(open.project.id(), &module) }))
  })
  .await
}

/// The module of project database `db` named by its id or, ignoring case, its name.
pub fn find_module(db: &ProjectDb, named: &str) -> Result<Module, ToolError> {
  let lowered = named.to_lowercase();
  db.modules()?
    .into_iter()
    .find(|module| module.id().to_string() == named || module.name().as_str().to_lowercase() == lowered)
    .ok_or_else(|| ToolError::not_found(Missing::Module, named))
}

fn form_details(form: &ModuleForm<'_>) -> Result<ModuleDetails, ToolError> {
  form
    .details()
    .map_err(|errors| ToolError::refused("invalid_module", errors.all()))
}

fn module_json(project: ProjectId, module: &Module) -> Value {
  let details = module.details();
  json!({
    "id": module.id().to_string(),
    "name": details.name().as_str(),
    "description": details.description(),
    "start_date": details.start().map(|day| day.to_string()),
    "target_date": details.target().map(|day| day.to_string()),
    "state": match details.state() {
      ModuleState::Open => "open",
      ModuleState::Closed => "closed",
    },
    "created_at": wire::time(module.created_at()),
    "link": Link::Module(project, module.id()).path(),
  })
}

fn progress_json(progress: Progress) -> Value {
  json!({
    "unit": progress.unit.name(),
    "scope": progress.scope,
    "done": progress.done,
    "remaining": progress.remaining(),
    "percent_done": progress.percent(),
  })
}

fn summary_json(project: ProjectId, summary: &ModuleSummary) -> Value {
  json!({
    "module": module_json(project, &summary.module),
    "tasks": summary.tasks,
    "progress": progress_json(summary.progress),
  })
}

fn page_json(project: ProjectId, page: &ModulePage) -> Value {
  let tasks: Vec<Value> = page
    .tasks
    .iter()
    .map(|task| {
      let key = task.key().to_string();
      json!({
        "key": key,
        "id": task.id().to_string(),
        "title": task.details().title.as_str(),
        "workflow_status": Workflow::from(task.organization().status()),
        "link": task_link(project, &key, TaskPage::Details),
      })
    })
    .collect();
  json!({
    "module": module_json(project, &page.module),
    "progress": progress_json(page.progress),
    "tasks": tasks,
    "burndown": burndown_json(&page.burndown),
  })
}

/// The chart by local day: a day still to come has no scope or remaining work, and a module without a target date
/// has no ideal line.
fn burndown_json(burndown: &Burndown) -> Value {
  let days: Vec<Value> = burndown
    .days
    .iter()
    .map(|day| {
      json!({
        "date": day.date.to_string(),
        "scope": day.scope,
        "remaining": day.remaining,
        "ideal": day.ideal.map(|ideal| (ideal * 100.0).round() / 100.0),
        "added": day.added,
        "removed": day.removed,
      })
    })
    .collect();
  json!({ "unit": burndown.unit.name(), "start_scope": burndown.start_scope, "days": days })
}
