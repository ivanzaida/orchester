//! One project's merge, worktree-removal and pull request operations, shared by all of its task pages
//! and tabs and by Orchester's MCP server. Git work continues after a page unmounts; its state and result
//! must have the same lifetime. Each project's state is published to subscribers
//! ([`IntegrationJobs::subscribe`]); what a window adds to it (a dialog, a button's width) is the
//! app's.

mod local;
mod merge;
mod pull_request;
mod remove;
mod resolve;
use std::{collections::HashMap, sync::Arc};

pub use local::LocalDecision;
pub use merge::MergeArgs;
use orchester_core::{
  id::{ProjectId, RunId},
  integration::RunMerge,
};
use parking_lot::Mutex;
pub use pull_request::{PullRequestArgs, PullRequestJob};
pub use remove::{RemoveArgs, Remover};
pub use resolve::{EditorOpened, ResolveArgs, ResolveJob, ResolveStep};
use tokio::sync::watch;

use crate::{activity::OperationSpec, messages::Message, workspace::OpenProject};

#[derive(Clone, Debug, Default, PartialEq)]
pub enum JobState {
  #[default]
  Idle,
  Running,
  Succeeded,
  Failed(Message),
}

/// The operation git runs for a run, one at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Active {
  Idle,
  Merge,
  Local,
  Remove,
  PullRequest,
  /// Merging the base branch into the run's worktree to resolve conflicts by hand, or finishing or aborting it.
  Resolve,
}

/// What git does for a run while the window closes: closing waits for it, or the person quits with it
/// shown (the closing panel). Every value is runtime data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BusyJob {
  pub project: Arc<str>,
  /// What it does, as the status bar says it while it runs ("Merging KON-1 run #1 into main").
  pub what: Message,
  pub merge: bool,
}

/// One run's operations as they stand.
#[derive(Clone, Debug, PartialEq)]
pub struct RunJobState {
  active: Active,
  local_decisions: usize,
  /// The operation under way, as the closing panel names it.
  busy_with: Option<BusyJob>,
  pub merge: JobState,
  pub remove: JobState,
  /// The files the last merge conflicted in, for the task page to list; empty unless it stopped at a
  /// conflict.
  pub conflicts: Vec<String>,
  /// What the last merge made of the run's work in git, also when it could not be recorded: the task
  /// page shows it while the task is not read again with it.
  pub merged: Option<RunMerge>,
  /// The last merge stopped where Local integration carries on
  /// ([`IntegrationProblem::continues_locally`](super::integration::IntegrationProblem::continues_locally)).
  pub continues_locally: bool,
  /// Creating a pull request from the run's branch.
  pub pull_request: PullRequestJob,
  /// Resolving the conflicts of the last merge by hand in the run's worktree.
  pub resolve: ResolveJob,
}

impl Default for RunJobState {
  fn default() -> Self {
    Self {
      active: Active::Idle,
      local_decisions: 0,
      busy_with: None,
      merge: JobState::Idle,
      remove: JobState::Idle,
      conflicts: Vec::new(),
      merged: None,
      continues_locally: false,
      pull_request: PullRequestJob::Idle,
      resolve: ResolveJob::default(),
    }
  }
}

impl RunJobState {
  /// Git merges the run, removes its worktrees or pushes its branch for a pull request.
  pub fn busy(&self) -> bool {
    self.active != Active::Idle
  }

  pub fn local_decision_pending(&self) -> bool {
    self.local_decisions > 0
  }

  /// Git removes the run's worktrees, whoever asked: a person, or automation.
  pub fn removing(&self) -> bool {
    self.active == Active::Remove
  }

  /// The run's branch is pushed and a pull request opened from it now.
  pub fn creating_pull_request(&self) -> bool {
    self.active == Active::PullRequest
  }

  /// Git merges into the run's worktree, or finishes or aborts that merge, to resolve conflicts by hand.
  pub fn resolving(&self) -> bool {
    self.active == Active::Resolve
  }
}

/// One project's operations as they stand: each run's that was asked about, and how often automation
/// removed worktrees.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProjectJobState {
  runs: HashMap<RunId, RunJobState>,
  removed_outside: u64,
}

impl ProjectJobState {
  pub fn local_decision_pending(&self) -> bool {
    self.runs.values().any(RunJobState::local_decision_pending)
  }

  pub fn run(&self, run: RunId) -> RunJobState {
    self.runs.get(&run).cloned().unwrap_or_default()
  }

  /// Each run that was asked about, with its operations.
  pub fn runs(&self) -> impl Iterator<Item = (RunId, &RunJobState)> {
    self.runs.iter().map(|(run, state)| (*run, state))
  }

  /// The run git merges now, if any: one merge at a time in a project's checkout.
  pub fn merging(&self) -> Option<RunId> {
    self
      .runs
      .iter()
      .find(|(_, state)| matches!(state.active, Active::Merge | Active::Local))
      .map(|(run, _)| *run)
  }

  /// Counts the removals Orchester's MCP server made: the task pages read what the runs left again.
  pub fn removed_outside(&self) -> u64 {
    self.removed_outside
  }
}

/// One project's operations. Clones share state, including across tabs of the project; every change
/// also counts in the revision its [`ProjectJobs`] shares.
#[derive(Clone)]
pub struct IntegrationJobs(watch::Sender<ProjectJobState>, watch::Sender<u64>);

impl Default for IntegrationJobs {
  fn default() -> Self {
    Self(watch::Sender::new(ProjectJobState::default()), watch::Sender::new(0))
  }
}

impl PartialEq for IntegrationJobs {
  fn eq(&self, other: &Self) -> bool {
    self.0.same_channel(&other.0)
  }
}

impl IntegrationJobs {
  pub fn subscribe(&self) -> watch::Receiver<ProjectJobState> {
    self.0.subscribe()
  }

  pub fn state(&self) -> ProjectJobState {
    self.0.borrow().clone()
  }

  pub fn of(&self, run: RunId) -> RunJobs {
    RunJobs {
      jobs: self.clone(),
      run,
    }
  }

  /// Records a removal automation made, after it changed what the run left.
  pub fn count_outside_removal(&self) {
    self.0.send_modify(|state| state.removed_outside += 1);
    self.changed();
  }

  fn changed(&self) {
    self.1.send_modify(|revision| *revision += 1);
  }
}

/// Every project's merge and removal operations, shared by the window's tabs and Orchester's MCP server,
/// so a removal a tool starts guards the task page's buttons and the other way round.
#[derive(Clone)]
pub struct ProjectJobs(Arc<Mutex<HashMap<ProjectId, IntegrationJobs>>>, watch::Sender<u64>);

impl Default for ProjectJobs {
  fn default() -> Self {
    Self(Arc::default(), watch::Sender::new(0))
  }
}

impl PartialEq for ProjectJobs {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl ProjectJobs {
  /// The operations of `project`, made the first time they are asked for.
  pub fn of(&self, project: ProjectId) -> IntegrationJobs {
    let revision = self.1.clone();
    self
      .0
      .lock()
      .entry(project)
      .or_insert_with(|| IntegrationJobs(watch::Sender::new(ProjectJobState::default()), revision))
      .clone()
  }

  /// Changes whenever an operation of any project changes.
  pub fn changes(&self) -> watch::Receiver<u64> {
    self.1.subscribe()
  }

  /// The merges and worktree removals under way in every project.
  pub fn busy(&self) -> Vec<BusyJob> {
    let projects: Vec<IntegrationJobs> = self.0.lock().values().cloned().collect();
    projects
      .iter()
      .flat_map(|jobs| {
        jobs
          .0
          .borrow()
          .runs
          .values()
          .filter_map(|run| run.busy_with.clone())
          .collect::<Vec<_>>()
      })
      .collect()
  }
}

/// One run's operations, a handle on its project's [`IntegrationJobs`].
#[derive(Clone)]
pub struct RunJobs {
  jobs: IntegrationJobs,
  run: RunId,
}

impl RunJobs {
  pub fn state(&self) -> RunJobState {
    self.jobs.0.borrow().run(self.run)
  }

  pub fn busy(&self) -> bool {
    self.state().busy()
  }

  fn update(&self, change: impl FnOnce(&mut RunJobState)) {
    self
      .jobs
      .0
      .send_modify(|state| change(state.runs.entry(self.run).or_default()));
    self.jobs.changed();
  }

  fn begin(&self, operation: Active) -> bool {
    let mut started = false;
    // Subscribers hear of a stale request that is refused too: what they show catches up with the
    // active operation instead of appearing to accept a second request. Completion is published
    // before the run is idle, and both are read under the one lock, so a stale request cannot see
    // the run idle again before its operation's success. Merges share the project's checkout: a merge
    // while another run's merge goes on is refused, and says why.
    self.jobs.0.send_modify(|project| {
      let other_merge =
        matches!(operation, Active::Merge | Active::Local) && project.merging().is_some_and(|run| run != self.run);
      let managed = operation == Active::Merge && project.local_decision_pending();
      let state = project.runs.entry(self.run).or_default();
      let completed = (operation == Active::Merge && state.merge == JobState::Succeeded)
        || (operation == Active::Remove && state.remove == JobState::Succeeded)
        || (operation == Active::PullRequest && matches!(state.pull_request, PullRequestJob::Created(_)));
      if state.active != Active::Idle || completed || managed {
        return;
      }
      if other_merge {
        state.merge = JobState::Failed(Message::new("runs.merge.failure.anotherMerge"));
        return;
      }
      state.active = operation;
      started = true;
    });
    started
  }

  fn finish(&self) {
    self.update(|state| {
      state.active = Active::Idle;
      state.busy_with = None;
    });
  }

  /// Names the operation that began, `what` it does, for the closing panel.
  fn name(&self, open: &OpenProject, what: &OperationSpec, merge: bool) {
    let busy = BusyJob {
      project: open.project.name().as_str().into(),
      what: what.running().clone(),
      merge,
    };
    self.update(|state| state.busy_with = Some(busy));
  }
}
