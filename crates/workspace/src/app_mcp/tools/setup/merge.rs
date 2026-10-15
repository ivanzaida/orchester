//! `get_changes` and `merge_run`: what a run changed, as the task's Changes tab reads it, and its work
//! merged as the task Summary's *Merge into main* merges it, through the same job: one merge or
//! removal at a time per task, the status bar's operation, the merge record and the screens told.

use std::{path::PathBuf, sync::Arc};

use orchester_core::{
  diff::{Diff, FileChange},
  id::RunId,
  integration::MergeKind,
  run::{Run, RunStatus},
  store::{MergeStore, RunStore, TaskStore},
};
use orchester_store::ProjectDb;
use serde_json::{Value, json};
use tokio::sync::watch;

use super::super::{
  Translate,
  runs::{InRun, find_run},
  wire,
};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  messages::Message,
  tasks::{
    changes::{ChangesError, Checkpointed, NotCheckpointed, RunStage, checkpointed_in, not_checkpointed_in},
    detail::run_store_error,
    integration_jobs::{JobState, MergeArgs, ProjectJobState},
    local_integration::{LegacyPolicy, LocalIntent, LocalOrigin, managed_checkout},
    outside::Outside,
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

pub async fn merge_run(services: &Services, args: InRun, translate: &Translate) -> Result<Value, ToolError> {
  let separator: Arc<str> = translate.text(&Message::new("common.listSeparator")).into();
  let started = {
    let services = services.clone();
    blocking(move || start(&services, &args, separator)).await?
  };
  let (mut updates, run, root) = started;
  let ended = updates
    .wait_for(|state| {
      let job = state.run(run);
      !job.busy() && matches!(job.merge, JobState::Succeeded | JobState::Failed(_))
    })
    .await
    .map(|state| state.run(run).merge)
    .map_err(|_| ToolError::interrupted())?;
  if let JobState::Failed(reason) = ended {
    return Err(ToolError::refused("merge_failed", vec![reason]));
  }
  blocking(move || {
    let merge = ProjectDb::open(&root)?
      .run_merge(run)?
      .ok_or_else(|| ToolError::refused("merge_failed", vec![Message::new("appMcp.errors.mergeNotRecorded")]))?;
    let kind = match merge.kind {
      MergeKind::FastForward => "fast_forward",
      MergeKind::MergeCommit => "merge_commit",
      MergeKind::AlreadyMerged => "already_merged",
    };
    Ok(json!({
      "merge": {
        "run": run.to_string(),
        "into": merge.into.to_string(),
        "revision": merge.revision.to_string(),
        "kind": kind,
        "merged_at": wire::time(merge.merged_at),
      }
    }))
  })
  .await
}

/// The project's merge and removal states to wait on, the run merging, and its repository.
type Started = (watch::Receiver<ProjectJobState>, RunId, PathBuf);

/// Checks that the Summary would offer *Merge into main* for the run, then starts the merge.
fn start(services: &Services, args: &InRun, separator: Arc<str>) -> Result<Started, ToolError> {
  let open = services.project(&args.project)?;
  let db = ProjectDb::open(&open.repo_root)?;
  let run = find_run(&db, &args.run)?;
  ready(&db, &run)?;
  let jobs = services.jobs.of(open.project.id());
  if managed_checkout(&db, run.id())? || jobs.state().local_decision_pending() {
    LocalOrigin::AuthenticatedDomain
      .authorize(LocalIntent::ReviewedDecision)
      .map_err(|_| refused("needs_decision", "localIntegration.personRequired"))?;
    return Err(refused("needs_decision", "localIntegration.personRequired"));
  }
  let policy = LegacyPolicy::new(&open.repo_root, run.id(), services.vault.clone())
    .map_err(|_| refused("integration_preflight_failed", "localIntegration.preflightFailed"))?;
  if let Some(evidence) = policy
    .preflight()
    .map_err(|_| refused("integration_preflight_failed", "localIntegration.preflightFailed"))?
  {
    return Err(refused(
      if evidence.kind == orchester_core::integration::local::ResultEvidenceKind::CompleteAncestry {
        "already_integrated"
      } else {
        "scoped_integration_required"
      },
      "localIntegration.externalResult",
    ));
  }
  let task_runs = db.runs_of_task(run.task()).map_err(run_store_error)?;
  let busy = task_runs.iter().any(|listed| {
    let id = listed.as_ref().map_or_else(|unreadable| unreadable.id, Run::id);
    jobs.of(id).busy()
  });
  if busy {
    return Err(refused("merge_running", "appMcp.errors.integrationRunning"));
  }
  let task = db
    .task(run.task())?
    .ok_or_else(|| ToolError::not_found(Missing::Task, run.task().to_string()))?;
  let branch = run
    .base_branch()
    .ok_or_else(|| refused("no_base_branch", "appMcp.errors.noBaseBranch"))?
    .to_string();
  let updates = jobs.subscribe();
  let (changes, project) = (services.changes.clone(), open.project.id());
  let started = jobs.of(run.id()).start_legacy_merge(
    MergeArgs {
      open: open.clone(),
      task: task.id(),
      key: task.key().to_string(),
      number: run.number().get(),
      branch,
      run: run.id(),
      programs: services.programs.clone(),
      activity: services.activity.clone(),
      separator,
      changed: Arc::new(move || changes.record(project, Outside::Tasks)),
    },
    policy,
  );
  // Another merge or removal of the run took it between the check above and now.
  if !started {
    return Err(refused("merge_running", "appMcp.errors.integrationRunning"));
  }
  Ok((updates, run.id(), open.repo_root))
}

/// Refused unless the run is its task's latest, succeeded, and was not merged yet.
fn ready(db: &ProjectDb, run: &Run) -> Result<(), ToolError> {
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
  if db.run_merge(run.id())?.is_some() {
    return Err(refused("already_merged", "appMcp.errors.alreadyMerged"));
  }
  Ok(())
}

fn refused(code: &str, key: &'static str) -> ToolError {
  ToolError::refused(code, vec![Message::new(key)])
}
