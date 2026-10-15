//! `list_tasks` and `get_task`: a project's tasks as the task list finds them (its search and its
//! workflow status filter), and one task as its page shows it, with its runs and their worktrees.

use std::collections::HashMap;

use orchester_core::{
  store::TaskStore,
  task::{Task, organization::TaskFilter},
  tokens::TokenBudget,
};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  Translate,
  across::across,
  wire::{self, Rank, Workflow, task_link},
  worktree::worktree_state,
};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  budget::whole_minutes,
  links::{Link, TaskPage},
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
  /// The project's id, or its name; left out, every project the app knows.
  #[serde(default)]
  pub project: Option<String>,
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
  let read = blocking(move || {
    // Git tells the runs merged outside Orchester for the project named; across projects the stored read suffices.
    let programs = project.is_some().then_some(&services.programs);
    across(&services, project.as_deref(), |open, db| {
      Ok(ProjectTasks::read(db, &open.repo_root, programs)?)
    })
  })
  .await?;
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
  Ok(read.json("tasks", translate, |open, data| {
    let project = open.project.id();
    browse::rows(data, TaskTab::All, query, TaskSort::Manual)
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
          "link": task_link(project, &row.key, TaskPage::Summary),
        })
      })
      .collect()
  }))
}

pub async fn get_task(services: &Services, args: GetTask, translate: &Translate) -> Result<Value, ToolError> {
  let services = services.clone();
  let (project, detail, tags, pipeline, worktrees) = blocking(move || {
    let open = services.project(&args.project)?;
    let root = open.repo_root;
    let db = ProjectDb::open(&root)?;
    let task = find_task(&db, &args.key)?;
    let tags = db.project_tags()?;
    let published = crate::tasks::data::published_pipelines(&db)?;
    let pipeline = PipelineLabel::of(task.details().pipeline, &published);
    let detail = TaskDetail::load_settled(&root, task.id(), &services.programs)?;
    let worktrees = detail
      .history
      .iter()
      .map(|record| Ok((record.id, worktree_state(&db, &root, record.id)?)))
      .collect::<Result<HashMap<_, _>, ToolError>>()?;
    Ok((open.project.id(), detail, tags, pipeline, worktrees))
  })
  .await?;
  let task = &detail.task;
  let link = Link::Task(project, task.key().clone(), TaskPage::Summary).path();
  let tag_names: Vec<&str> = tags
    .iter()
    .filter(|tag| task.organization().tags().contains(&tag.id()))
    .map(|tag| tag.name().as_str())
    .collect();
  let runs: Vec<Value> = detail
    .history
    .iter()
    .map(|record| run_record(record, worktrees.get(&record.id).copied(), translate, &link))
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
    "limits": {
      "budget_tokens": task.details().limits.budget().map(TokenBudget::get),
      "time_limit_minutes": task.details().limits.max_duration().map(whole_minutes),
    },
    "phase": wire::phase(detail.phase()),
    "runs": runs,
    "link": link,
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

/// A run of the task as its Summary lists it; `link` is the task's Summary, where its runs are.
fn run_record(record: &RunRecord, worktree: Option<&str>, translate: &Translate, link: &str) -> Value {
  json!({
    "id": record.id.to_string(),
    "worktree": worktree,
    "number": record.number.map(|number| number.get()),
    "phase": record.phase.map(wire::phase),
    "detail": translate.text(&record.detail),
    "changed_at": record.at.map(wire::time),
    "link": link,
  })
}
