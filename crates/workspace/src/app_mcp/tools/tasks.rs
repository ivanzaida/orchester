//! `list_tasks` and `get_task`: a project's tasks as the task list finds them (its search and its
//! workflow status filter), and one task as its page shows it, with its runs and their worktrees.

use std::collections::HashMap;

use orchester_core::{
  store::TaskStore,
  task::{Task, organization::TaskFilter},
};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  Translate,
  wire::{self, Rank, Workflow},
  worktree::worktree_state,
};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  tasks::{
    browse::{self, PipelineFilter, Query, TaskSort},
    data::ProjectTasks,
    detail::TaskDetail,
    history::RunRecord,
    list::{PipelineLabel, TaskTab},
  },
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct ListTasks {
  /// The project's id, or its name.
  pub project: String,
  /// Only tasks with this workflow status.
  #[serde(default)]
  pub status: Option<Workflow>,
  /// Words the title, key, id or brief contain, as the task list's search matches them.
  #[serde(default)]
  pub search: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct GetTask {
  /// The project's id, or its name.
  pub project: String,
  /// The task's key, e.g. `KON-12`, or its id.
  pub key: String,
}

/// Joins the keys of a row's blocking tasks.
const SEPARATOR: &str = ", ";

pub async fn list_tasks(services: &Services, args: ListTasks, translate: &Translate) -> Result<Value, ToolError> {
  let services = services.clone();
  let project = args.project.clone();
  let data = blocking(move || Ok(ProjectTasks::load(&services.project(&project)?.repo_root)?)).await?;
  let filter = TaskFilter {
    statuses: args.status.into_iter().map(Into::into).collect(),
    ..TaskFilter::default()
  };
  let query = Query {
    text: args.search.as_deref().unwrap_or_default(),
    pipeline: PipelineFilter(None),
    organization: &filter,
    separator: SEPARATOR,
  };
  let tasks: Vec<Value> = browse::rows(&data, TaskTab::All, query, TaskSort::Manual)
    .iter()
    .map(|row| {
      json!({
        "key": &*row.key,
        "id": row.id.to_string(),
        "title": &*row.title,
        "workflow_status": Workflow::from(row.organization.status()),
        "priority": Rank::from(row.organization.priority()),
        "tags": row.tags.iter().map(|tag| &**tag).collect::<Vec<_>>(),
        "pipeline": translate.text(&row.pipeline.message()),
        "phase": row.phase.map(wire::phase),
        "detail": translate.text(&row.detail),
        "created_at": wire::time(row.created_at),
      })
    })
    .collect();
  Ok(json!({ "tasks": tasks }))
}

pub async fn get_task(services: &Services, args: GetTask, translate: &Translate) -> Result<Value, ToolError> {
  let services = services.clone();
  let (detail, tags, pipeline, worktrees) = blocking(move || {
    let root = services.project(&args.project)?.repo_root;
    let db = ProjectDb::open(&root)?;
    let task = find_task(&db, &args.key)?;
    let tags = db.project_tags()?;
    let published = crate::tasks::data::published_pipelines(&db)?;
    let pipeline = PipelineLabel::of(task.details().pipeline, &published);
    let detail = TaskDetail::load_in(&root, task.id())?;
    let worktrees = detail
      .history
      .iter()
      .map(|record| Ok((record.id, worktree_state(&db, &root, record.id)?)))
      .collect::<Result<HashMap<_, _>, ToolError>>()?;
    Ok((detail, tags, pipeline, worktrees))
  })
  .await?;
  let task = &detail.task;
  let tag_names: Vec<&str> = tags
    .iter()
    .filter(|tag| task.organization().tags().contains(&tag.id()))
    .map(|tag| tag.name().as_str())
    .collect();
  let runs: Vec<Value> = detail
    .history
    .iter()
    .map(|record| run_record(record, worktrees.get(&record.id).copied(), translate))
    .collect();
  Ok(json!({
    "key": task.key().to_string(),
    "id": task.id().to_string(),
    "title": task.details().title.as_str(),
    "brief": &task.details().brief,
    "criteria": &task.details().acceptance_criteria,
    "submission": wire::submission(task.status()),
    "workflow_status": Workflow::from(task.organization().status()),
    "priority": Rank::from(task.organization().priority()),
    "tags": tag_names,
    "blocked_reason": task.organization().blocked_reason(),
    "blocked_by": &detail.blocked_by,
    "pipeline": {
      "id": task.details().pipeline.map(|id| id.to_string()),
      "shown": translate.text(&pipeline.message()),
      "pinned_version": task.pipeline_version().map(|version| version.get()),
    },
    "limits": task.details().limits.map(|limits| json!({
      "budget_tokens": limits.budget().get(),
      "time_limit_minutes": limits.max_duration().as_secs() / 60,
    })),
    "phase": wire::phase(detail.phase()),
    "runs": runs,
  }))
}

/// The task of project database `db` named by its key, ignoring case, or its id.
pub fn find_task(db: &ProjectDb, named: &str) -> Result<Task, ToolError> {
  let lowered = named.to_lowercase();
  db.tasks()?
    .into_iter()
    .find(|task| task.key().to_string().to_lowercase() == lowered || task.id().to_string() == named)
    .ok_or_else(|| ToolError::not_found(Missing::Task, named))
}

fn run_record(record: &RunRecord, worktree: Option<&str>, translate: &Translate) -> Value {
  json!({
    "id": record.id.to_string(),
    "worktree": worktree,
    "number": record.number.map(|number| number.get()),
    "phase": record.phase.map(wire::phase),
    "detail": translate.text(&record.detail),
    "changed_at": record.at.map(wire::time),
  })
}
