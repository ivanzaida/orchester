//! One project's merge and worktree-removal operations as its task pages show them: what
//! `orchester_workspace::tasks::integration_jobs` reports, mirrored into signals by the project's
//! feed loader ([`RunIntegrations::publish`]), and what only the window has (the merge button's
//! width, the removal dialog).

use std::{collections::HashMap, sync::Arc};

use lurq::core::Signal;
use orchester_core::id::RunId;
pub use orchester_workspace::tasks::integration_jobs::{
  IntegrationJobs, JobState, MergeArgs, ProjectJobState, ProjectJobs, RemoveArgs, Remover, RunJobState, RunJobs,
};
use parking_lot::Mutex;

use crate::tabs::feed::publish;

struct RunView {
  job: RunJobs,
  state: Signal<RunJobState>,
  merge: Signal<JobState>,
  merge_width: Signal<f32>,
  remove: Signal<JobState>,
  dialog_open: Signal<bool>,
  discard: Signal<bool>,
}

/// One run's operations. Clones share state, including across tabs of its project.
#[derive(Clone)]
pub struct RunIntegration(Arc<RunView>);

impl PartialEq for RunIntegration {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl lurq::app::component::DevtoolsInspectable for RunIntegration {}

impl RunIntegration {
  fn new(job: RunJobs) -> Self {
    let state = job.state();
    Self(Arc::new(RunView {
      merge: Signal::new(state.merge.clone()),
      remove: Signal::new(state.remove.clone()),
      state: Signal::new(state),
      merge_width: Signal::new(140.0),
      dialog_open: Signal::new(false),
      discard: Signal::new(false),
      job,
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

  /// Git merges or removes for the run, as the view shows it; a render reads this.
  pub fn busy(&self) -> bool {
    self.0.state.get().busy()
  }

  /// How the run's operations stand now, for a click to decide by: the view may be a frame behind.
  pub fn state_now(&self) -> RunJobState {
    self.0.job.state()
  }

  /// Git removes the run's worktrees, whoever asked: a person, or automation.
  pub fn removing(&self) -> bool {
    self.0.state.get().removing()
  }

  /// Starts the merge and shows at once what it changed, before the feed loader passes it on.
  pub fn start_merge(&self, args: MergeArgs) {
    self.0.job.start_merge(args);
    self.mirror(&self.0.job.state());
  }

  /// Starts the removal and shows at once what it changed, before the feed loader passes it on.
  pub fn start_remove(&self, args: RemoveArgs) {
    self.0.job.start_remove(args);
    self.mirror(&self.0.job.state());
  }

  /// A person's removal from this run's dialog, which closes once the worktrees are gone.
  pub fn person(&self) -> Remover {
    let dialog_open = self.0.dialog_open.clone();
    Remover::Person(Arc::new(move || dialog_open.set(false)))
  }

  /// Opens the confirmation of *Remove worktree* anew: discarding not agreed to, and an earlier
  /// failure, which stayed under the run's row once the dialog closed, cleared.
  pub fn open_remove(&self) {
    if matches!(self.0.job.state().remove, JobState::Failed(_)) {
      self.0.job.clear_failed_removal();
      self.mirror(&self.0.job.state());
    }
    self.0.discard.set(false);
    self.0.dialog_open.set(true);
  }

  /// Closes the confirmation unless git is removing; a failure stays to be shown under the run's
  /// row in *Runs of this task* (63.06 `VBR27`).
  pub fn dismiss_remove(&self) {
    if self.0.job.state().remove == JobState::Running {
      return;
    }
    self.0.dialog_open.set(false);
  }

  /// Shows `state`: the outcome first, then whether git is still at work, as the job publishes them.
  fn mirror(&self, state: &RunJobState) {
    publish(&self.0.merge, state.merge.clone());
    publish(&self.0.remove, state.remove.clone());
    publish(&self.0.state, state.clone());
  }
}

/// One project's runs as its task pages show them, shared by its tabs.
#[derive(Clone)]
pub struct RunIntegrations {
  jobs: IntegrationJobs,
  runs: Arc<Mutex<HashMap<RunId, RunIntegration>>>,
  removed_outside: Signal<u64>,
}

impl RunIntegrations {
  pub fn new(jobs: IntegrationJobs) -> Self {
    let removed_outside = Signal::new(jobs.state().removed_outside());
    Self {
      jobs,
      runs: Arc::default(),
      removed_outside,
    }
  }

  /// What the project's feed loader follows.
  pub fn jobs(&self) -> &IntegrationJobs {
    &self.jobs
  }

  /// Counts the removals Orchester's MCP server made: the task pages read what the runs left again.
  pub fn removed_outside(&self) -> Signal<u64> {
    self.removed_outside.clone()
  }

  pub fn of(&self, run: RunId) -> RunIntegration {
    self
      .runs
      .lock()
      .entry(run)
      .or_insert_with(|| RunIntegration::new(self.jobs.of(run)))
      .clone()
  }

  /// Shows `state`, as the project's feed loader received it.
  pub fn publish(&self, state: &ProjectJobState) {
    // Copied out first: a signal's readers may ask for a run while it is set.
    let shown: Vec<(RunId, RunIntegration)> = self
      .runs
      .lock()
      .iter()
      .map(|(run, view)| (*run, view.clone()))
      .collect();
    for (run, view) in shown {
      view.mirror(&state.run(run));
    }
    publish(&self.removed_outside, state.removed_outside());
  }
}
