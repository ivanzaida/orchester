//! One project's merge and worktree-removal operations, shared by all of its task pages and tabs.
//! Git work continues after a page unmounts; its state and result must have the same lifetime.

use std::{
  collections::HashMap,
  panic::{AssertUnwindSafe, catch_unwind},
  sync::Arc,
  thread,
};

use lurq::core::Signal;
use orchester_core::{id::RunId, integration::Leftovers};
use parking_lot::Mutex;

use super::integration::{merge_in, remove_in};
use crate::{
  activity::{Activity, operations},
  messages::{self, Message},
  programs::Programs,
  workspace::OpenProject,
};

#[derive(Clone, Debug, PartialEq)]
pub enum JobState {
  Idle,
  Running,
  Succeeded,
  Failed(Message),
}

impl lurq::app::component::DevtoolsInspectable for JobState {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Active {
  Idle,
  Merge,
  Remove,
}

impl lurq::app::component::DevtoolsInspectable for Active {}

struct RunJobs {
  active: Signal<Active>,
  merge: Signal<JobState>,
  merge_width: Signal<f32>,
  remove: Signal<JobState>,
  dialog_open: Signal<bool>,
  discard: Signal<bool>,
}

/// One run's operations. Clones share state, including across tabs of its project.
#[derive(Clone)]
pub struct RunIntegration(Arc<RunJobs>);

impl PartialEq for RunIntegration {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl lurq::app::component::DevtoolsInspectable for RunIntegration {}

impl RunIntegration {
  fn new() -> Self {
    Self(Arc::new(RunJobs {
      active: Signal::new(Active::Idle),
      merge: Signal::new(JobState::Idle),
      merge_width: Signal::new(140.0),
      remove: Signal::new(JobState::Idle),
      dialog_open: Signal::new(false),
      discard: Signal::new(false),
    }))
  }

  pub fn merge(&self) -> Signal<JobState> {
    self.0.merge.clone()
  }

  pub fn merge_width(&self) -> Signal<f32> {
    self.0.merge_width.clone()
  }

  pub fn remove(&self) -> Signal<JobState> {
    self.0.remove.clone()
  }

  pub fn dialog_open(&self) -> Signal<bool> {
    self.0.dialog_open.clone()
  }

  pub fn discard(&self) -> Signal<bool> {
    self.0.discard.clone()
  }

  pub fn busy(&self) -> bool {
    self.0.active.get() != Active::Idle
  }

  fn begin(&self, operation: Active) -> bool {
    let mut started = false;
    // Notify readers even when a stale click is refused: the rendered control catches up with
    // the active operation instead of appearing to accept a second request.
    self.0.active.update(|active| {
      // Completion is published before Active becomes Idle. Read it only while holding Active's
      // write lock, or a stale click can observe Running, pause, then enter after Idle.
      let completed = (operation == Active::Merge && self.0.merge.get_untracked() == JobState::Succeeded)
        || (operation == Active::Remove && self.0.remove.get_untracked() == JobState::Succeeded);
      if *active == Active::Idle && !completed {
        *active = operation;
        started = true;
      }
    });
    started
  }

  fn finish(&self) {
    self.0.active.set(Active::Idle);
  }

  pub fn start_merge(&self, args: MergeArgs) {
    if !self.begin(Active::Merge) {
      return;
    }
    self.0.merge.set(JobState::Running);
    let job = self.clone();
    if let Err(error) = thread::Builder::new().name("run-merge".into()).spawn(move || {
      let MergeArgs {
        open,
        task,
        key,
        number,
        branch,
        run,
        programs,
        activity,
        separator,
        changed,
      } = args;
      let running = activity.start(operations::run_merge(&open, task, (&key, number), &branch));
      let not_done = Message::new("statusBar.operations.runMerge.notDone")
        .with("task", key)
        .with("run", number.to_string());
      let result = catch_unwind(AssertUnwindSafe(|| merge_in(&open.repo_root, &programs, run)));
      let state = match result {
        Ok(Ok(_)) => {
          running.finish();
          JobState::Succeeded
        }
        Ok(Err(problem)) => {
          let message = problem.message(&separator);
          problem.end(running, not_done);
          JobState::Failed(message)
        }
        Err(_) => {
          tracing::error!("run merge thread panicked");
          running.fail(messages::interrupted());
          JobState::Failed(messages::interrupted())
        }
      };
      job.0.merge.set(state);
      job.finish();
      changed();
    }) {
      tracing::error!(%error, "cannot start run merge thread");
      self.0.merge.set(JobState::Failed(messages::interrupted()));
      self.finish();
    }
  }

  pub fn start_remove(&self, args: RemoveArgs) {
    if !self.begin(Active::Remove) {
      return;
    }
    self.0.remove.set(JobState::Running);
    let job = self.clone();
    if let Err(error) = thread::Builder::new().name("run-cleanup".into()).spawn(move || {
      let RemoveArgs {
        open,
        task,
        key,
        number,
        run,
        confirmed,
        programs,
        activity,
        separator,
        changed,
      } = args;
      let running = activity.start(operations::run_cleanup(&open, task, (&key, number)));
      let not_done = Message::new("statusBar.operations.runCleanup.notDone")
        .with("task", key)
        .with("run", number.to_string());
      let result = catch_unwind(AssertUnwindSafe(|| {
        remove_in(&open.repo_root, &programs, &run, &confirmed)
      }));
      let state = match result {
        Ok(Ok(())) => {
          running.finish();
          job.0.dialog_open.set(false);
          JobState::Succeeded
        }
        Ok(Err(problem)) => {
          let message = problem.message(&separator);
          problem.end(running, not_done);
          JobState::Failed(message)
        }
        Err(_) => {
          tracing::error!("run cleanup thread panicked");
          running.fail(messages::interrupted());
          JobState::Failed(messages::interrupted())
        }
      };
      job.0.remove.set(state);
      job.finish();
      changed();
    }) {
      tracing::error!(%error, "cannot start run cleanup thread");
      self.0.remove.set(JobState::Failed(messages::interrupted()));
      self.finish();
    }
  }

  pub fn dismiss_remove(&self) {
    if self.0.remove.get_untracked() == JobState::Running {
      return;
    }
    self.0.dialog_open.set(false);
    if matches!(self.0.remove.get_untracked(), JobState::Failed(_)) {
      self.0.remove.set(JobState::Idle);
    }
  }
}

#[derive(Clone)]
pub struct MergeArgs {
  pub open: OpenProject,
  pub task: orchester_core::id::TaskId,
  pub key: String,
  pub number: u32,
  pub branch: String,
  pub run: RunId,
  pub programs: Programs,
  pub activity: Activity,
  pub separator: Arc<str>,
  pub changed: Arc<dyn Fn() + Send + Sync>,
}

#[derive(Clone)]
pub struct RemoveArgs {
  pub open: OpenProject,
  pub task: orchester_core::id::TaskId,
  pub key: String,
  pub number: u32,
  pub run: orchester_core::run::Run,
  pub confirmed: Leftovers,
  pub programs: Programs,
  pub activity: Activity,
  pub separator: Arc<str>,
  pub changed: Arc<dyn Fn() + Send + Sync>,
}

/// Project-scoped registry retained while any tab still names the project.
#[derive(Clone, Default)]
pub struct IntegrationJobs(Arc<Mutex<HashMap<RunId, RunIntegration>>>);

impl IntegrationJobs {
  pub fn of(&self, run: RunId) -> RunIntegration {
    self.0.lock().entry(run).or_insert_with(RunIntegration::new).clone()
  }
}
