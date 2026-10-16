//! Which task a pull request is of: the task whose run made the pull request's head branch. A run names its
//! branch when it starts (`orchester_core::run::Run::branch`), from its task's key, so a pull request opened
//! from a run's branch of this repository, by a person or a step, is that run's. A fork's branch of the same
//! name is not: a pull request from another repository has no task. Known gap: the key names a task of this
//! project's database, so a project set up again, or the same repository on another computer, can give a key
//! to another task than the one whose run pushed the branch.

use std::{
  collections::{HashMap, HashSet},
  sync::Arc,
};

use orchester_core::{
  id::{RunId, TaskId},
  project::TaskKey,
  pull_request::PullRequest,
  run::RunNumber,
  store::{RunStore, TaskStore},
};
use orchester_store::{ProjectDb, StoreError};

use crate::tasks::detail::run_store_error;

/// The task and run a pull request's head branch is of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedTask {
  pub task: TaskId,
  pub key: TaskKey,
  pub title: Arc<str>,
  pub run: RunId,
  pub run_number: RunNumber,
  /// The run is running and no pause was asked of it: an agent works on the pull request's branch now.
  pub agent_working: bool,
}

/// The task of each of `pull_requests` that has one, by pull request number.
pub fn linked_tasks(db: &ProjectDb, pull_requests: &[PullRequest]) -> Result<HashMap<u64, LinkedTask>, StoreError> {
  let ours: Vec<&PullRequest> = pull_requests
    .iter()
    .filter(|pull_request| !pull_request.is_cross_repository())
    .collect();
  if ours.is_empty() {
    return Ok(HashMap::new());
  }
  // A run's branch is its own: the latest run of a branch, should one be named again, is the one listed.
  let by_branch: HashMap<String, _> = db
    .run_branches()?
    .into_iter()
    .map(|run| (run.branch.as_str().to_owned(), run))
    .collect();
  let tasks: HashMap<TaskId, _> = db.tasks()?.into_iter().map(|task| (task.id(), task)).collect();
  let working = working_runs(db)?;
  Ok(
    ours
      .into_iter()
      .filter_map(|pull_request| {
        let run = by_branch.get(pull_request.head().as_str())?;
        let task = tasks.get(&run.task)?;
        let linked = LinkedTask {
          task: task.id(),
          key: task.key().clone(),
          title: Arc::from(task.details().title.as_str()),
          run: run.run,
          run_number: run.number,
          agent_working: working.contains(&run.run),
        };
        Some((pull_request.number(), linked))
      })
      .collect(),
  )
}

/// The runs recorded as running with no pause asked of them. A run whose record cannot be read is not
/// counted: its task page says why.
fn working_runs(db: &ProjectDb) -> Result<HashSet<RunId>, StoreError> {
  Ok(
    db.running_runs()
      .map_err(run_store_error)?
      .into_iter()
      .filter_map(Result::ok)
      .filter(|run| !run.pause_requested())
      .map(|run| run.id())
      .collect(),
  )
}
