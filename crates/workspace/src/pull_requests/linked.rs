//! Which task a pull request is of: the task whose run made the pull request's head branch. A run
//! names its branch when it starts (`orchester_core::run::Run::branch`) and no other run takes the
//! same name, so a pull request opened from a run's branch, by a person or a step, is that run's.

use std::{collections::HashMap, sync::Arc};

use orchester_core::{
  id::{RunId, TaskId},
  project::TaskKey,
  pull_request::PullRequest,
  run::{Run, RunNumber, RunStatus},
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
  /// The run is running: an agent works on the pull request's branch now.
  pub agent_working: bool,
}

/// The task of each of `pull_requests` that has one, by pull request number.
pub fn linked_tasks(db: &ProjectDb, pull_requests: &[PullRequest]) -> Result<HashMap<u64, LinkedTask>, StoreError> {
  if pull_requests.is_empty() {
    return Ok(HashMap::new());
  }
  let by_branch = runs_by_branch(db)?;
  Ok(
    pull_requests
      .iter()
      .filter_map(|pull_request| {
        let linked = by_branch.get(pull_request.head().as_str())?;
        Some((pull_request.number(), linked.clone()))
      })
      .collect(),
  )
}

/// Every readable run's task by the run's branch; of runs that share a branch, the latest.
fn runs_by_branch(db: &ProjectDb) -> Result<HashMap<String, LinkedTask>, StoreError> {
  let mut by_branch: HashMap<String, LinkedTask> = HashMap::new();
  for task in db.tasks()? {
    for listed in db.runs_of_task(task.id()).map_err(run_store_error)? {
      // A run whose record cannot be read has no branch to match; the task page says why.
      let Ok(run) = listed else {
        continue;
      };
      let linked = LinkedTask {
        task: task.id(),
        key: task.key().clone(),
        title: Arc::from(task.details().title.as_str()),
        run: run.id(),
        run_number: run.number(),
        agent_working: working(&run),
      };
      let branch = run.branch().as_str().to_owned();
      let later = by_branch
        .get(&branch)
        .is_none_or(|known| known.run_number.get() < linked.run_number.get());
      if later {
        by_branch.insert(branch, linked);
      }
    }
  }
  Ok(by_branch)
}

fn working(run: &Run) -> bool {
  matches!(run.status(), RunStatus::Running)
}
