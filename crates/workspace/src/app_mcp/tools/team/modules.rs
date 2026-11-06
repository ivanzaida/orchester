//! `list_modules`, `get_module`, `create_module` and `update_module` of a team project: its modules as the team server
//! reads them, with their branch, progress and claim, and made and changed through the dispatcher on the module's
//! Admin-checked routes, so a member's change is refused `not_admin`. A module's branch is the team's, assigned by an
//! Admin: no base branch is kept on this computer for it.

use orchester_api::{
  SharedModule,
  v2::{ModuleListRequest, ModuleSummary, Page, PageRequest},
};
use orchester_core::{id::ModuleId, module::ModuleState};
use serde_json::{Value, json};

use super::{
  Team, on_worker,
  outcome::{accepted, action_refused},
  read, read_refused,
};
use crate::{
  actions::{Accepted, Action, ActionOutcome, Definition, Intent, ModuleAction},
  app_mcp::{
    Services,
    error::{Missing, ToolError},
    tools::modules::{CreateModule, UpdateModule, form_details},
  },
  links::Link,
  messages::Message,
  modules::form::ModuleForm,
  team_reads::{TeamProject, TeamReadError},
};

pub(in crate::app_mcp::tools) async fn list_modules(services: &Services, team: &Team) -> Result<Value, ToolError> {
  let project = team.reads(services).await?;
  let id = team.open.project.id();
  let modules: Vec<Value> = all_modules(&project)
    .await?
    .iter()
    .map(|summary| summary_json(id, summary))
    .collect();
  Ok(json!({ "modules": modules, "link": Link::Modules(id).path(), "team": true }))
}

pub(in crate::app_mcp::tools) async fn get_module(
  services: &Services,
  team: &Team,
  named: &str,
) -> Result<Value, ToolError> {
  let project = team.reads(services).await?;
  let summary = find(&project, named).await?;
  Ok(summary_json(team.open.project.id(), &summary))
}

pub(in crate::app_mcp::tools) async fn create_module(
  services: &Services,
  team: &Team,
  args: &CreateModule,
) -> Result<Value, ToolError> {
  if args.base_branch.is_some() {
    return Err(branch_is_teams());
  }
  let form = ModuleForm {
    name: &args.name,
    description: args.description.as_deref().unwrap_or_default(),
    start: args.start_date.as_deref().unwrap_or_default(),
    target: args.target_date.as_deref().unwrap_or_default(),
    state: ModuleState::Open,
  };
  let details = form_details(&form)?;
  let module = changed(team, services, ModuleAction::Create { details }).await?;
  Ok(json!({ "module": module_json(team.open.project.id(), &module) }))
}

pub(in crate::app_mcp::tools) async fn update_module(
  services: &Services,
  team: &Team,
  args: &UpdateModule,
) -> Result<Value, ToolError> {
  if args.base_branch.is_some() {
    return Err(branch_is_teams());
  }
  let current = find(&team.reads(services).await?, &args.module).await?.module.module;
  let base = current.fields.details().map_err(|_| unreadable())?;
  let day = |given: &Option<Option<String>>, stored: Option<chrono::NaiveDate>| match given {
    Some(value) => value.clone().unwrap_or_default(),
    None => stored.map(|day| day.to_string()).unwrap_or_default(),
  };
  let (start, target) = (
    day(&args.start_date, base.start()),
    day(&args.target_date, base.target()),
  );
  let form = ModuleForm {
    name: args.name.as_deref().unwrap_or(base.name().as_str()),
    description: args.description.as_deref().unwrap_or(base.description()),
    start: &start,
    target: &target,
    state: args.state.map_or(base.state(), Into::into),
  };
  let edited = form_details(&form)?;
  let action = ModuleAction::Edit {
    module: current.id,
    read: current.revision,
    base,
    edited,
  };
  let module = changed(team, services, action).await?;
  Ok(json!({ "module": module_json(team.open.project.id(), &module) }))
}

/// `action` through the project's dispatcher: the module as the server holds it after it.
async fn changed(team: &Team, services: &Services, action: ModuleAction) -> Result<SharedModule, ToolError> {
  let dispatcher = team.dispatcher(services).await?;
  let intent = Intent::new(Action::Module(action));
  let outcome = on_worker(move || async move { dispatcher.dispatch(&intent).await.map_err(action_refused) }).await?;
  let ActionOutcome::Team(outcome) = outcome else {
    return Err(unreadable());
  };
  match accepted(outcome)? {
    Accepted::Definition(Definition::Module(module)) => Ok(*module),
    _ => Err(unreadable()),
  }
}

/// The module `named` by its id or, ignoring case, its name.
async fn find(project: &TeamProject, named: &str) -> Result<ModuleSummary, ToolError> {
  if let Ok(id) = named.parse::<ModuleId>() {
    return match read(project, project.module(id)).await {
      Ok(summary) => Ok(summary.into_value()),
      Err(TeamReadError::NotFound) => Err(ToolError::not_found(Missing::Module, named)),
      Err(error) => Err(read_refused(&error)),
    };
  }
  let lowered = named.to_lowercase();
  all_modules(project)
    .await?
    .into_iter()
    .find(|summary| summary.module.module.fields.name.to_lowercase() == lowered)
    .ok_or_else(|| ToolError::not_found(Missing::Module, named))
}

/// Every module of the project, open and closed, following the listing's pages.
async fn all_modules(project: &TeamProject) -> Result<Vec<ModuleSummary>, ToolError> {
  let mut modules = Vec::new();
  let mut page = PageRequest::first();
  loop {
    let request = ModuleListRequest {
      state: None,
      page: page.clone(),
    };
    let Page { items, next, .. } = read(project, project.module_page(request))
      .await
      .map_err(|error| read_refused(&error))?
      .into_value();
    modules.extend(items);
    match next {
      Some(next) => page = PageRequest::new(Some(next), None).expect("the default page limit is in range"),
      None => return Ok(modules),
    }
  }
}

fn module_json(project: orchester_core::id::ProjectId, module: &SharedModule) -> Value {
  let fields = &module.fields;
  json!({
    "id": module.id.to_string(),
    "name": fields.name,
    "description": fields.description,
    "start_date": fields.start,
    "target_date": fields.target,
    "state": match fields.state {
      ModuleState::Open => "open",
      ModuleState::Closed => "closed",
    },
    "revision": module.revision.get(),
    "team": true,
    "link": Link::Module(project, module.id).path(),
  })
}

fn summary_json(project: orchester_core::id::ProjectId, summary: &ModuleSummary) -> Value {
  let read = &summary.module;
  json!({
    "module": module_json(project, &read.module),
    "branch": read.branch,
    "assignee": read.assignee,
    "claim": read.effective_claim,
    "work": read.work,
    "tasks": summary.tasks,
    "progress": summary.progress,
    "forecast": summary.forecast,
  })
}

/// A team module's branch is the team's, assigned by an Admin; this computer keeps none of its own.
fn branch_is_teams() -> ToolError {
  ToolError::refused(
    "team_field_unsupported",
    vec![Message::new("appMcp.errors.teamFieldUnsupported").with("fields", "base_branch".to_owned())],
  )
}

/// The server answered with a module this version cannot read, or with something else than a module.
fn unreadable() -> ToolError {
  ToolError::refused("team_read_failed", vec![Message::new("appMcp.errors.teamReadFailed")])
}
