//! `get_changes` and `merge_run`: what a run changed, as the task's Changes tab reads it, and its work
//! merged as the task Summary's *Merge into main* merges it, through the same job: one merge or
//! removal at a time per task, the status bar's operation, the merge record and the screens told.

use orchester_core::{
  diff::{Diff, FileChange},
  run::{Run, RunStatus},
  store::{MergeStore, RunStore},
};
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use super::super::{
  Translate,
  runs::{InRun, find_run},
};
use crate::{
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  messages::Message,
  tasks::{
    changes::{ChangesError, Checkpointed, NotCheckpointed, RunStage, checkpointed_in, not_checkpointed_in},
    detail::run_store_error,
  },
};

pub async fn get_changes(services: &Services, args: InRun) -> Result<Value, ToolError> {
  let services = services.clone();
  let unreadable = |error: ChangesError| ToolError::refused("cannot_read_changes", vec![error.message()]);
  blocking(move || {
    let open = services.project(&args.project)?;
    let run = find_run(&ProjectDb::open(&open.repo_root)?, &args.run)?;
    let head = run.last_checkpoint().map(|checkpoint| checkpoint.revision().clone());
    let checkpointed =
      checkpointed_in(&open.repo_root, &services.programs, run.base(), head.as_ref()).map_err(unreadable)?;
    let stage = RunStage::of(run.status());
    let worktree = not_checkpointed_in(&open.repo_root, &services.programs, (run.id(), stage)).map_err(unreadable)?;
    Ok(json!({
      "run": { "id": run.id().to_string(), "number": run.number().get() },
      "checkpointed": checkpointed_json(&checkpointed),
      "not_checkpointed": not_checkpointed_json(&worktree),
    }))
  })
  .await
}

fn checkpointed_json(changes: &Checkpointed) -> Value {
  match changes {
    Checkpointed::NoCheckpoint => json!({ "state": "no_checkpoint" }),
    Checkpointed::Read { head, changes } => {
      json!({ "state": "read", "from": changes.from.to_string(), "to": head.to_string(), "diff": diff_json(&changes.diff) })
    }
    Checkpointed::TooLarge { head, size } => json!({
      "state": "too_large",
      "to": head.to_string(),
      "files": size.files,
      "additions": size.additions,
      "deletions": size.deletions,
    }),
  }
}

fn not_checkpointed_json(changes: &NotCheckpointed) -> Value {
  match changes {
    NotCheckpointed::Clean => json!({ "state": "clean" }),
    NotCheckpointed::Removed => json!({ "state": "worktree_removed" }),
    NotCheckpointed::Read(changes) => {
      json!({ "state": "read", "from": changes.from.to_string(), "diff": diff_json(&changes.diff) })
    }
    NotCheckpointed::TooLarge { from, size } => json!({
      "state": "too_large",
      "from": from.to_string(),
      "files": size.files,
      "additions": size.additions,
      "deletions": size.deletions,
    }),
  }
}

fn diff_json(diff: &Diff) -> Value {
  let files: Vec<Value> = diff
    .files()
    .iter()
    .map(|file| {
      let change = match file.change() {
        FileChange::Added => "added",
        FileChange::Deleted => "deleted",
        FileChange::Modified => "modified",
        FileChange::Renamed { .. } => "renamed",
        FileChange::Copied { .. } => "copied",
      };
      json!({
        "path": file.path(),
        "old_path": file.old_path(),
        "change": change,
        "binary": file.is_binary(),
        "additions": file.additions(),
        "deletions": file.deletions(),
      })
    })
    .collect();
  json!({ "files": files, "additions": diff.additions(), "deletions": diff.deletions() })
}

/// MCP cannot answer the checkout and exact-result decisions reserved for a person.
pub async fn merge_run(services: &Services, args: InRun, _translate: &Translate) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let db = ProjectDb::open(&open.repo_root)?;
    let run = find_run(&db, &args.run)?;
    ready(&db, &run)?;
    let jobs = services.jobs.of(open.project.id());
    let busy = db
      .runs_of_task(run.task())
      .map_err(run_store_error)?
      .iter()
      .any(|listed| {
        let id = listed.as_ref().map_or_else(|unreadable| unreadable.id, Run::id);
        jobs.of(id).busy()
      });
    if busy {
      return Err(refused("merge_running", "appMcp.errors.integrationRunning"));
    }
    Err(refused("person_required", "localIntegration.personRequired"))
  })
  .await
}

/// Refused unless the run is its task's latest, succeeded, and was not merged yet.
fn ready(db: &ProjectDb, run: &Run) -> Result<(), ToolError> {
  if db.run_merge(run.id())?.is_some() {
    return Err(refused("already_merged", "appMcp.errors.alreadyMerged"));
  }
  if !matches!(run.status(), RunStatus::Succeeded) {
    return Err(refused("run_not_ready", "appMcp.errors.runNotReady"));
  }
  let latest = db
    .runs_of_task(run.task())
    .map_err(run_store_error)?
    .iter()
    // A later run that cannot be read is still a later run.
    .filter_map(|listed| match listed {
      Ok(listed) => Some(listed.number().get()),
      Err(unreadable) => unreadable.number.map(|number| number.get()),
    })
    .max();
  if latest != Some(run.number().get()) {
    return Err(refused("not_latest_run", "appMcp.errors.notLatestRun"));
  }
  Ok(())
}

fn refused(code: &str, key: &'static str) -> ToolError {
  ToolError::refused(code, vec![Message::new(key)])
}
