//! Proposals of the Orchestrator chat a person's decision claimed and never settled: the app closed or failed while
//! the decision was carried out ([`super::proposal::decide`] runs detached from the window, so only the end of the
//! process cuts one short). At the start of a session no decision of an earlier one is running, so each claim made
//! before the session started is settled by what really happened where that can be read: a merge recorded for its
//! run, a run cancelled, a version published, a run started for the task, is taken as done; anything else is refused
//! as interrupted, and nothing more happens to it.

use std::path::Path;

use orchester_core::{
  id::{ResourceId, RunId},
  orchestrator::{
    ActionOutcome, ChatAction,
    change::{Change, Publishing},
  },
  resource::{AgentProfile, Instruction, Pipeline, ResourceBody, Skill, Tool},
  run::RunStatus,
  store::{MergeStore, ResourceStore, RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use crate::{
  app_mcp::Translate,
  messages::{self, Message},
  runner::Runners,
  tasks::detail::run_store_error,
  workspace::Workspace,
};

/// Settles the claims made before `before` in the project at `root`, the reason of an interrupted one in the words
/// of `translate`; how many it settled. Blocks on SQLite.
pub fn recover_in(root: &Path, before: Timestamp, translate: &Translate) -> Result<usize, Message> {
  let mut db = ProjectDb::open(root).map_err(|error| messages::store_error(&error))?;
  let claimed = db
    .claimed_chat_actions(before)
    .map_err(|error| messages::store_error(&error))?;
  let mut settled = 0;
  for (chat, seq, action) in claimed {
    let outcome = match carried_out(&db, &action).map_err(|error| messages::store_error(&error))? {
      Some(outcome) => outcome,
      None => ActionOutcome::Refused {
        code: "interrupted".to_owned(),
        reason: translate.text(&Message::new("orchestrator.proposal.interrupted")),
      },
    };
    let at = Timestamp::now();
    if db
      .finish_chat_action((chat, seq), (outcome, None), at)
      .map_err(|error| messages::store_error(&error))?
      .is_some()
    {
      settled += 1;
    }
  }
  Ok(settled)
}

/// Settles the claims an earlier session left in every registered project whose folder is there, and tells the
/// chats that changed. A project that cannot be read is logged and left for the next start.
pub async fn recover_all(workspace: Workspace, (before, translate): (Timestamp, Translate), runners: Runners) {
  let recovered = tokio::task::spawn_blocking(move || {
    let projects = match workspace.projects() {
      Ok(projects) => projects,
      Err(error) => {
        tracing::error!(%error, "cannot list the projects to recover the Orchestrator's decisions");
        return Vec::new();
      }
    };
    let mut changed = Vec::new();
    for project in projects.iter().filter(|project| !project.folder_missing()) {
      match recover_in(&project.repo_root, before, &translate) {
        Ok(0) => {}
        Ok(_) => changed.push(project.repo_root.clone()),
        Err(error) => tracing::warn!(project = %project.id, ?error, "cannot recover the Orchestrator's decisions"),
      }
    }
    changed
  })
  .await;
  match recovered {
    Ok(changed) => changed.iter().for_each(|root| runners.chat_changed(root)),
    Err(error) => tracing::error!(%error, "recovering the Orchestrator's decisions did not finish"),
  }
}

/// How the decision on `action` went, as it can be read: `None` when it cannot be seen to have been carried out.
fn carried_out(db: &ProjectDb, action: &ChatAction) -> Result<Option<ActionOutcome>, StoreError> {
  let applied = match (&action.outcome, action.change.as_ref().map(|change| &change.change)) {
    (_, Some(Change::Merge { run, .. })) => db.run_merge(*run)?.is_some(),
    (_, Some(Change::Cancel { run, .. })) => cancelled(db, *run)?,
    // A run that went on to a newer attempt than the one it stopped in was continued.
    (_, Some(Change::Continue { run, attempt, .. })) => went_on(db, *run, *attempt)?,
    (
      _,
      Some(Change::Publish {
        of, resource, version, ..
      }),
    ) => latest(db, *of, *resource)? >= *version,
    (ActionOutcome::Proposed(_), None) => {
      let started = action.claimed.unwrap_or(Timestamp::from_unix_millis(0));
      return Ok(started_since(db, action.task.as_deref(), started)?.then_some(ActionOutcome::Done));
    }
    _ => false,
  };
  Ok(applied.then_some(ActionOutcome::Applied))
}

fn cancelled(db: &ProjectDb, run: RunId) -> Result<bool, StoreError> {
  Ok(
    db.run(run)
      .map_err(run_store_error)?
      .is_some_and(|run| *run.status() == RunStatus::Cancelled),
  )
}

/// Whether run `run` is in a newer attempt than `attempt`.
fn went_on(db: &ProjectDb, run: RunId, attempt: u32) -> Result<bool, StoreError> {
  Ok(
    db.run(run)
      .map_err(run_store_error)?
      .and_then(|run| run.current_attempt().map(|current| current.number().get()))
      .is_some_and(|current| current > attempt),
  )
}

/// Whether task `key` has a run created at `since` or later.
fn started_since(db: &ProjectDb, key: Option<&str>, since: Timestamp) -> Result<bool, StoreError> {
  let Some(key) = key else {
    return Ok(false);
  };
  let Some(task) = db.tasks()?.into_iter().find(|task| task.key().to_string() == key) else {
    return Ok(false);
  };
  Ok(
    db.runs_of_task(task.id())
      .map_err(run_store_error)?
      .into_iter()
      .flatten()
      .any(|run| run.created_at() >= since),
  )
}

/// The number of the latest published version of `resource`, of kind `of`; 0 before its first.
fn latest(db: &ProjectDb, of: Publishing, resource: ResourceId) -> Result<u32, StoreError> {
  match of {
    Publishing::Pipeline => latest_of::<Pipeline>(db, resource),
    Publishing::Profile => latest_of::<AgentProfile>(db, resource),
    Publishing::Instruction => latest_of::<Instruction>(db, resource),
    Publishing::Skill => latest_of::<Skill>(db, resource),
    Publishing::Tool => latest_of::<Tool>(db, resource),
  }
}

fn latest_of<B: ResourceBody>(db: &ProjectDb, resource: ResourceId) -> Result<u32, StoreError> {
  Ok(
    db.latest_version::<B>(resource)?
      .map_or(0, |version| version.number().get()),
  )
}
