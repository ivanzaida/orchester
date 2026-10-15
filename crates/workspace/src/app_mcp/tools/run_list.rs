//! `list_runs`: the runs of a task, of a project or of every project the app knows, newest first, each as its
//! task's Summary lists it (*Runs of this task*): where it stands, what happened and its link.

use std::cmp::Reverse;

use orchester_core::{id::ProjectId, run::RunNumber, store::TaskStore, task::Task, tokens::TokenBudget};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  Translate,
  across::{across, needs_project},
  tasks::find_task,
  wire,
};
use crate::{
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  links::{Link, TaskPage},
  tasks::history::{RunRecord, runs_of},
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct ListRuns {
  /// The project's id, or its name; left out, every project the app knows.
  #[serde(default)]
  pub project: Option<String>,
  /// Only this task's runs: its key, e.g. `KON-12`, or its id. Needs `project`.
  #[serde(default)]
  pub task: Option<String>,
}

pub async fn list_runs(services: &Services, args: ListRuns, translate: &Translate) -> Result<Value, ToolError> {
  if args.task.is_some() && args.project.is_none() {
    return Err(needs_project("task"));
  }
  let services = services.clone();
  let read = blocking(move || {
    across(&services, args.project.as_deref(), |open| {
      let db = ProjectDb::open(&open.repo_root)?;
      let tasks = match &args.task {
        Some(named) => vec![find_task(&db, named)?],
        None => db.tasks()?,
      };
      tasks
        .into_iter()
        .map(|task| {
          let runs = runs_of(&db, &task)?;
          Ok((task, runs))
        })
        .collect::<Result<Vec<_>, ToolError>>()
    })
  })
  .await?;
  let mut runs = read.items(|open, tasks| {
    tasks
      .iter()
      .flat_map(|(task, runs)| runs.iter().map(|run| run_json(open.project.id(), task, run, translate)))
      .collect()
  });
  // Newest first across tasks and projects; a run that cannot be read has no time and comes last.
  runs.sort_by_key(|run| Reverse(run["created_at"].as_i64()));
  Ok(json!({ "runs": runs, "unreadable_projects": read.unreadable(translate) }))
}

/// A run of `task` of `project` as the task's Summary lists it, with its state, its tokens and its link: the
/// task's page. A run that cannot be read has its id, number and why alone.
fn run_json(project: ProjectId, task: &Task, record: &RunRecord, translate: &Translate) -> Value {
  let run = record.run.as_deref();
  json!({
    "id": record.id.to_string(),
    "number": record.number.map(RunNumber::get),
    "task": { "key": task.key().to_string(), "title": task.details().title.as_str() },
    "phase": record.phase.map(wire::phase),
    "status": run.map(|run| wire::run_status(run.status())),
    "detail": translate.text(&record.detail),
    "tokens": run.map(|run| json!({ "used": run.used().get(), "budget": run.budget().map(TokenBudget::get) })),
    "created_at": run.map(|run| wire::time(run.created_at())),
    "changed_at": record.at.map(wire::time),
    "link": Link::Task(project, task.key().clone(), TaskPage::Summary).path(),
  })
}
