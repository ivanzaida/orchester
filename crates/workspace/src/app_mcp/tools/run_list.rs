//! `list_runs`: the runs of a task, of a project or of every project the app knows, newest first, each as its
//! task's Summary lists it (*Runs of this task*): where it stands, its place in the merge queue, what happened and its
//! link.

use std::{
  cmp::Reverse,
  collections::{HashMap, HashSet},
};

use orchester_core::{
  id::{ProjectId, RunId},
  run::RunNumber,
  store::TaskStore,
  task::Task,
  tokens::TokenBudget,
};
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  Translate,
  across::{Across, across, needs_project},
  merge_queue::deliveries,
  tasks::find_task,
  wire,
};
use crate::{
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  links::{Link, TaskPage},
  tasks::{
    delivery::settle_records,
    history::{RunRecord, runs_of},
  },
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
  /// How many of the newest runs to give: 1 to 200; left out, 50.
  #[serde(default)]
  pub limit: Option<usize>,
}

/// How many runs `list_runs` gives when the call does not say, and the most it gives.
pub const DEFAULT_LIMIT: usize = 50;
pub const MAX_LIMIT: usize = 200;

pub async fn list_runs(services: &Services, args: ListRuns, translate: &Translate) -> Result<Value, ToolError> {
  if args.task.is_some() && args.project.is_none() {
    return Err(needs_project("task"));
  }
  let limit = args.limit.unwrap_or(DEFAULT_LIMIT);
  if !(1..=MAX_LIMIT).contains(&limit) {
    return Err(ToolError::InvalidArguments(format!(
      "`limit` is 1 to {MAX_LIMIT}, not {limit}"
    )));
  }
  let services = services.clone();
  let read = blocking(move || {
    let mut read = across(&services, args.project.as_deref(), |_, db| {
      let tasks = match &args.task {
        Some(named) => vec![find_task(db, named)?],
        None => db.tasks()?,
      };
      let tasks = tasks
        .into_iter()
        .map(|task| {
          let runs = runs_of(db, &task)?;
          Ok((task, runs))
        })
        .collect::<Result<Vec<_>, ToolError>>()?;
      Ok((tasks, deliveries(db)?))
    })?;
    // Git tells the runs merged outside Orchester, as the task's Summary reads them: only for the runs given, and
    // once for each project.
    let given = newest(&read, limit);
    read.each_mut(|open, (tasks, _)| {
      let runs = tasks
        .iter_mut()
        .flat_map(|(_, runs)| runs.iter_mut())
        .filter(|run| given.contains(&run.id));
      settle_records(&services.programs, &open.repo_root, runs);
    });
    Ok(read)
  })
  .await?;
  let mut runs = read.items(|open, (tasks, deliveries)| {
    tasks
      .iter()
      .flat_map(|(task, runs)| {
        runs
          .iter()
          .map(|run| run_json(open.project.id(), (task, run, deliveries.get(&run.id)), translate))
      })
      .collect()
  });
  // Newest first across tasks and projects; a run that cannot be read has no time and comes last.
  runs.sort_by_key(|run| Reverse(run["created_at"].as_i64()));
  let more = runs.len() > limit;
  runs.truncate(limit);
  Ok(json!({ "runs": runs, "more": more, "unreadable_projects": read.unreadable(translate) }))
}

/// A project's tasks with their runs, and the place of each run that joined the merge queue.
type Listed = (Vec<(Task, Vec<RunRecord>)>, HashMap<RunId, Value>);

/// The `limit` runs of `read` the answer gives, newest first as it sorts them: the items' order, then by when they
/// started, a run that cannot be read last.
fn newest(read: &Across<Listed>, limit: usize) -> HashSet<RunId> {
  let mut runs: Vec<(RunId, Option<i64>)> = read
    .found()
    .flat_map(|(_, (tasks, _))| tasks.iter().flat_map(|(_, runs)| runs.iter()))
    .map(|run| (run.id, run.run.as_ref().map(|run| wire::time(run.created_at()))))
    .collect();
  runs.sort_by_key(|(_, created)| Reverse(*created));
  runs.into_iter().take(limit).map(|(id, _)| id).collect()
}

/// A run of `task` of `project` as the task's Summary lists it, with its state, its place in the merge queue
/// (`delivery`, when it joined it), its tokens and its link: the task's page. A run that cannot be read has its id,
/// number and why alone.
fn run_json(
  project: ProjectId,
  (task, record, delivery): (&Task, &RunRecord, Option<&Value>),
  translate: &Translate,
) -> Value {
  let run = record.run.as_deref();
  json!({
    "id": record.id.to_string(),
    "number": record.number.map(RunNumber::get),
    "task": { "key": task.key().to_string(), "title": task.details().title.as_str() },
    "phase": record.phase.map(wire::phase),
    "status": run.map(|run| wire::run_status(run.status(), record.runner_failure)),
    "delivery": delivery,
    "detail": translate.text(&record.detail),
    "tokens": run.map(|run| json!({ "used": run.used().get(), "budget": run.budget().map(TokenBudget::get) })),
    "created_at": run.map(|run| wire::time(run.created_at())),
    "changed_at": record.at.map(wire::time),
    "link": Link::Task(project, task.key().clone(), TaskPage::Summary).path(),
  })
}
