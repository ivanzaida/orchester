//! `remove_worktree`: removes an ended run's worktree as the task page's *Remove worktree* does (ORC-63),
//! through the same job: one merge or removal at a time per task, the status bar's operation, the
//! removal record and the screens told. Where the page's confirmation would make the person agree to
//! discard uncommitted changes, the tool refuses instead; it offers no way to force it. Also each run's
//! worktree as `get_task` and `get_run` show it.

use std::{path::Path, sync::Arc, thread, time::Duration};

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
    integration_jobs::{JobState, RemoveArgs, RunIntegration},
  },
};

/// How often the tool looks whether the removal it started has ended.
const POLL: Duration = Duration::from_millis(50);

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
    let job = jobs.of(run.id());
    job.clear_failed_remove();
    let (changes, removed) = (services.changes.clone(), jobs.removed_outside());
    job.start_remove(RemoveArgs {
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
        changes.record();
        removed.update(|count| *count += 1);
      }),
    });
    finished(&job)?;
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

/// Waits for the removal `job` started; refused when it did not start (another operation took the run
/// first) or failed, with the reason the task page shows.
fn finished(job: &RunIntegration) -> Result<(), ToolError> {
  loop {
    match job.remove().get_untracked() {
      JobState::Running => thread::sleep(POLL),
      JobState::Succeeded => return Ok(()),
      JobState::Failed(reason) => return Err(ToolError::refused("removal_failed", vec![reason])),
      JobState::Idle => return Err(refused("merge_running", "appMcp.errors.integrationRunning")),
    }
  }
}
