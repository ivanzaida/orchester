//! Creating a pull request from a run's branch on the project's shared operations, so it goes on when the task
//! page closes ([`crate::pull_requests::create`]). One operation of the run at a time: its merge waits while the
//! branch is pushed. Closing the window does not wait for it: pushing and opening change nothing in the checkout.
//! A run that has its pull request is not given a second one.

use std::{
  panic::{AssertUnwindSafe, catch_unwind},
  sync::Arc,
  thread,
};

use orchester_core::id::{RunId, TaskId};
use orchester_exec::programs::Programs;

use super::{Active, RunJobs};
use crate::{
  activity::{Activity, operations},
  messages::Message,
  pull_requests::create::{CreateProblem, Created, create_in},
  workspace::OpenProject,
};

/// Creating a pull request for a run, as it stands.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum PullRequestJob {
  #[default]
  Idle,
  /// The branch is pushed and the pull request opened now.
  Running,
  Created(Created),
  Failed(CreateProblem),
}

/// What creating a pull request needs.
#[derive(Clone)]
pub struct PullRequestArgs {
  pub open: OpenProject,
  pub task: TaskId,
  pub key: String,
  pub number: u32,
  pub run: RunId,
  /// Opens it as a draft.
  pub draft: bool,
  pub programs: Programs,
  pub activity: Activity,
  /// Called once it ended, so the task page reads the pull requests again.
  pub changed: Arc<dyn Fn() + Send + Sync>,
}

impl RunJobs {
  /// Starts creating the run's pull request on a thread of its own; whether it began, which it does not while
  /// another operation of the run works or once a pull request was created.
  pub fn start_pull_request(&self, args: PullRequestArgs) -> bool {
    if !self.begin(Active::PullRequest) {
      return false;
    }
    self.update(|state| state.pull_request = PullRequestJob::Running);
    let job = self.clone();
    if let Err(error) = thread::Builder::new()
      .name("run-pull-request".into())
      .spawn(move || job.create_now(args))
    {
      tracing::error!(%error, "cannot start the pull request thread");
      let problem = CreateProblem::Store(crate::messages::interrupted());
      self.update(|state| state.pull_request = PullRequestJob::Failed(problem));
      self.finish();
    }
    true
  }

  fn create_now(&self, args: PullRequestArgs) {
    let PullRequestArgs {
      open,
      task,
      key,
      number,
      run,
      draft,
      programs,
      activity,
      changed,
    } = args;
    let running = activity.start(operations::run_pull_request(&open, task, (&key, number)));
    let result = catch_unwind(AssertUnwindSafe(|| create_in(&open.repo_root, &programs, (run, draft))));
    if let Ok(Err(problem)) = &result {
      tracing::warn!(?problem, %run, "no pull request was created for the run");
    }
    let job = match result {
      Ok(Ok(created)) => {
        running.finish();
        PullRequestJob::Created(created)
      }
      Ok(Err(CreateProblem::Store(message))) => {
        running.fail(message.clone());
        PullRequestJob::Failed(CreateProblem::Store(message))
      }
      // The task page says why in place.
      Ok(Err(problem)) => {
        running.finish_as(
          Message::new("statusBar.operations.runPullRequest.notDone")
            .with("task", key)
            .with("run", number.to_string()),
        );
        PullRequestJob::Failed(problem)
      }
      Err(_) => {
        tracing::error!("pull request thread panicked");
        running.fail(crate::messages::interrupted());
        PullRequestJob::Failed(CreateProblem::Store(crate::messages::interrupted()))
      }
    };
    self.update(|state| state.pull_request = job);
    self.finish();
    changed();
  }
}
