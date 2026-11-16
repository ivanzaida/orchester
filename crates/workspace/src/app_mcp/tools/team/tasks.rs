//! `list_tasks` and `get_task` of a team project: its tasks as the team server reads them (S10), with their claim,
//! work and assignee. A task is named by its key or its server id, never looked up in this computer's copy.

use orchester_api::v2::{Page, PageRequest, SearchText, TaskFilters, TaskListRequest, TaskRead};
use orchester_core::id::TaskId;
use serde_json::{Value, json};

use super::{Team, read, read_refused};
use crate::{
  app_mcp::{
    Services,
    error::{Missing, ToolError},
    tools::wire::{self, Rank, Workflow, task_link},
  },
  links::TaskPage,
  team_reads::{TeamProject, TeamReadError},
};

/// How many tasks a page of the listing holds.
const PAGE: u16 = 100;

/// Every task of `team` its `filters` keep, in the list's own order, read page by page from one snapshot.
pub(in crate::app_mcp::tools) async fn list_tasks(
  services: &Services,
  team: &Team,
  filters: TaskFilters,
) -> Result<Value, ToolError> {
  let project = team.reads(services).await?;
  let tasks = all_tasks(&project, filters)
    .await
    .map_err(|error| read_refused(&error))?;
  let id = team.open.project.id();
  let items: Vec<Value> = tasks.iter().map(|task| listed(id, task)).collect();
  Ok(json!({ "tasks": items, "unreadable_projects": [] }))
}

/// Task `key` of `team`, by its key or its server id.
pub(in crate::app_mcp::tools) async fn get_task(
  services: &Services,
  team: &Team,
  key: &str,
) -> Result<Value, ToolError> {
  let project = team.reads(services).await?;
  let task = find(&project, key).await?;
  Ok(detail(team.open.project.id(), &task))
}

/// Task `key` of the team project `project`: by its server id, or by its key among the tasks a search for it finds.
pub(super) async fn find(project: &TeamProject, key: &str) -> Result<TaskRead, ToolError> {
  let missing = || ToolError::not_found(Missing::Task, key);
  if let Ok(id) = key.parse::<TaskId>() {
    return match read(project, project.task(id)).await {
      Ok(task) => Ok(task.into_value()),
      Err(TeamReadError::NotFound) => Err(missing()),
      Err(error) => Err(read_refused(&error)),
    };
  }
  let search = SearchText::new(key).map_err(|_| missing())?;
  let filters = TaskFilters {
    search: Some(search),
    ..TaskFilters::default()
  };
  let lowered = key.to_lowercase();
  all_tasks(project, filters)
    .await
    .map_err(|error| read_refused(&error))?
    .into_iter()
    .find(|task| task.task.key.to_lowercase() == lowered)
    .ok_or_else(missing)
}

/// Every task `filters` keep, following the listing's pages.
async fn all_tasks(project: &TeamProject, filters: TaskFilters) -> Result<Vec<TaskRead>, TeamReadError> {
  let mut tasks = Vec::new();
  let mut after = None;
  loop {
    let request = TaskListRequest {
      filters: filters.clone(),
      page: PageRequest::new(after, Some(PAGE)).expect("the page limit is in range"),
      ..TaskListRequest::default()
    };
    let Page { items, next, .. } = read(project, project.task_page(request)).await?.into_value();
    tasks.extend(items);
    match next {
      Some(next) => after = Some(next),
      None => return Ok(tasks),
    }
  }
}

/// A task as `list_tasks` lists it.
fn listed(project: orchester_core::id::ProjectId, read: &TaskRead) -> Value {
  let task = &read.task;
  json!({
    "key": task.key,
    "id": task.id.to_string(),
    "title": task.content.title,
    "workflow_status": Workflow::from(task.content.workflow),
    "priority": Rank::from(task.content.priority),
    "tags": task.content.tags,
    "blocked_reason": task.content.blocked_reason,
    "module": task.content.module.map(|module| module.to_string()),
    "assignee": read.assignee,
    "claim": read.effective_claim,
    "work": read.work,
    "team": true,
    "link": task_link(project, &task.key, TaskPage::Summary),
  })
}

/// A task as `get_task` gives it: what the team server holds of it, at its revision. `closed` and `blocked` are the
/// server's records of who closed it or set its reason, `null` when it has none (an older server sends none).
fn detail(project: orchester_core::id::ProjectId, read: &TaskRead) -> Value {
  let task = &read.task;
  let content = &task.content;
  json!({
    "key": task.key,
    "id": task.id.to_string(),
    "revision": task.revision.get(),
    "title": content.title,
    "brief": content.brief,
    "criteria": content.criteria,
    "submission": wire::submission(task.status),
    "workflow_status": Workflow::from(content.workflow),
    "priority": Rank::from(content.priority),
    "tags": content.tags,
    "module": content.module.map(|module| module.to_string()),
    "blocked_reason": content.blocked_reason,
    "blocked_by": content.blocking,
    "pipeline": { "id": content.pipeline },
    "limits": content.limits.as_ref().map(|limits| json!({
      "budget_tokens": limits.budget_tokens,
      "time_limit_minutes": limits.max_duration_ms.map(|ms| ms.div_ceil(60_000)),
    })),
    "assignee": read.assignee,
    "claim": read.effective_claim,
    "work": read.work,
    "closed": read.closed,
    "blocked": read.blocked,
    "capabilities": read.capabilities,
    "team": true,
    "link": task_link(project, &task.key, TaskPage::Summary),
  })
}
