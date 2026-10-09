//! `remove_worktree`: removes an ended run's worktree as the task page's *Remove worktree* does (ORC-63),
//! through the same job: one merge or removal at a time per task, the status bar's operation, the
//! removal record and the screens told. Where the page's confirmation would make the person agree to
//! discard uncommitted changes, the tool refuses instead; it offers no way to force it. Also each run's
//! worktree as `get_task` and `get_run` show it.

use std::{
  path::Path,
  sync::{Arc, mpsc},
};

use orchester_core::{
  id::RunId,
  store::{RunStore, TaskStore, WorktreeStore},
};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Translate, runs::find_run};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  messages::Message,
  tasks::{
    changes::run_worktree,
    detail::run_store_error,
    integration::leftovers_in,
    integration_jobs::{RemoveArgs, Remover},
    outside::Outside,
  },
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct RemoveWorktree {
  /// The project's id, or its name.
  pub project: String,
  /// The run's id, or its number (`7` or `#7`).
  pub run: String,
}

/// A run's worktree: `present`, `removed` (a person or a tool removed it), or `none`.
pub fn worktree_state(db: &ProjectDb, repo_root: &Path, run: RunId) -> Result<&'static str, ToolError> {
  if db.worktree_removal(run)?.is_some() {
    return Ok("removed");
  }
  Ok(if run_worktree(repo_root, run).is_dir() {
    "present"
  } else {
    "none"
  })
}

fn refused(code: &str, key: &'static str) -> ToolError {
  ToolError::refused(code, vec![Message::new(key)])
}

pub async fn remove_worktree(
  services: &Services,
  args: RemoveWorktree,
  translate: &Translate,
) -> Result<Value, ToolError> {
  let separator: Arc<str> = translate.text(&Message::new("common.listSeparator")).into();
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let db = ProjectDb::open(&open.repo_root)?;
    let run = find_run(&db, &args.run)?;
    if run.status().has_live_work() {
      return Err(refused("run_not_ended", "appMcp.errors.runNotEnded"));
    }
    let jobs = services.jobs.of(open.project.id());
    // One merge or removal at a time per task, as the task page has it.
    let task_runs: Vec<RunId> = db
      .runs_of_task(run.task())
      .map_err(run_store_error)?
      .iter()
      .map(|listed| listed.as_ref().map_or_else(|unreadable| unreadable.id, |run| run.id()))
      .collect();
    if task_runs.iter().any(|id| jobs.of(*id).busy()) {
      return Err(refused("merge_running", "appMcp.errors.integrationRunning"));
    }
    if db.worktree_removal(run.id())?.is_some() {
      return Err(refused("already_removed", "appMcp.errors.worktreeRemoved"));
    }
    let left = leftovers_in(&open.repo_root, &services.programs, &[Arc::new(run.clone())])
      .map_err(|problem| ToolError::refused("cannot_read_worktree", vec![problem.message(&separator)]))?
      .0
      .into_iter()
      .next()
      .map(|(_, left)| left)
      .filter(|left| !left.worktrees.is_empty())
      .ok_or_else(|| refused("no_worktree", "appMcp.errors.noWorktree"))?;
    if left.loses_files() {
      return Err(refused("uncommitted_changes", "appMcp.errors.uncommittedChanges"));
    }
    let task = db
      .task(run.task())?
      .ok_or_else(|| ToolError::not_found(Missing::Task, run.task().to_string()))?;
    let count = left.worktrees.len();
    let (answer, outcome) = mpsc::channel();
    let (changes, removed, project) = (services.changes.clone(), jobs.clone(), open.project.id());
    jobs.of(run.id()).start_remove(RemoveArgs {
      open: open.clone(),
      task: task.id(),
      key: task.key().to_string(),
      number: run.number().get(),
      run: run.clone(),
      confirmed: left,
      programs: services.programs.clone(),
      activity: services.activity.clone(),
      separator,
      changed: Arc::new(move || {
        changes.record(project, Outside::Tasks);
        removed.count_outside_removal();
      }),
      by: Remover::Automation(answer),
    });
    // The removal drops the channel unanswered when it did not start: another operation took the run.
    match outcome.recv() {
      Ok(Ok(())) => {}
      Ok(Err(reason)) => return Err(ToolError::refused("removal_failed", vec![reason])),
      Err(_) => return Err(refused("merge_running", "appMcp.errors.integrationRunning")),
    }
    Ok(json!({
      "run": {
        "id": run.id().to_string(),
        "number": run.number().get(),
        "worktree": worktree_state(&db, &open.repo_root, run.id())?,
      },
      "worktrees_removed": count,
    }))
  })
  .await
}
