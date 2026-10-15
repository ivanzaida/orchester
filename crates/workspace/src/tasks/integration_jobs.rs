//! One project's merge and worktree-removal operations, shared by all of its task pages and tabs and
//! by Orchester's MCP server. Git work continues after a page unmounts; its state and result must
//! have the same lifetime. Each project's state is published to subscribers
//! ([`IntegrationJobs::subscribe`]); what a window adds to it (a dialog, a button's width) is the
//! app's.

mod local;
use std::{
  collections::HashMap,
  panic::{AssertUnwindSafe, catch_unwind},
  sync::{Arc, mpsc},
  thread,
};

pub use local::LocalDecision;
use orchester_core::{
  id::{ProjectId, RunId},
  integration::{Leftovers, MergeError, RunMerge},
  revision::Revision,
};
use orchester_exec::programs::Programs;
use parking_lot::Mutex;
use tokio::sync::watch;

use super::integration::{IntegrationProblem, merge_in, remove_in};
use crate::{
  activity::{Activity, OperationSpec, operations},
  messages::{self, Message},
  workspace::OpenProject,
};

#[derive(Clone, Debug, PartialEq)]
pub enum JobState {
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
    }
  }
}

impl RunJobState {
  /// Git merges the run or removes its worktrees.
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
        || (operation == Active::Remove && state.remove == JobState::Succeeded);
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

  /// Starts merging the run on a thread of its own; whether it began, which it does not while another
  /// operation of the run works or once its merge succeeded.
  pub fn start_merge(&self, args: MergeArgs) -> bool {
    self.start_merge_with(args, (None, None))
  }

  pub fn start_legacy_merge(&self, args: MergeArgs, policy: super::local_integration::LegacyPolicy) -> bool {
    self.start_merge_with(args, (Some(policy), None))
  }

  /// [`Self::start_legacy_merge`], only while the run's branch is at `expected`, the commit a person reviewed.
  pub fn start_legacy_merge_at(
    &self,
    args: MergeArgs,
    policy: super::local_integration::LegacyPolicy,
    expected: Revision,
  ) -> bool {
    self.start_merge_with(args, (Some(policy), Some(expected)))
  }

  fn start_merge_with(
    &self,
    args: MergeArgs,
    (policy, expected): (Option<super::local_integration::LegacyPolicy>, Option<Revision>),
  ) -> bool {
    if !self.begin(Active::Merge) {
      return false;
    }
    self.update(|state| {
      state.merge = JobState::Running;
      state.conflicts.clear();
      state.merged = None;
    });
    let what = operations::run_merge(&args.open, args.task, (&args.key, args.number), &args.branch);
    self.name(&args.open, &what, true);
    let job = self.clone();
    if let Err(error) = thread::Builder::new()
      .name("run-merge".into())
      .spawn(move || job.merge_now(args, (policy.as_ref(), expected.as_ref())))
    {
      tracing::error!(%error, "cannot start run merge thread");
      self.update(|state| state.merge = JobState::Failed(messages::interrupted()));
      self.finish();
    }
    true
  }

  fn merge_now(
    &self,
    args: MergeArgs,
    (policy, expected): (Option<&super::local_integration::LegacyPolicy>, Option<&Revision>),
  ) {
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
    let result = catch_unwind(AssertUnwindSafe(|| {
      if let Some(policy) = &policy {
        match policy.preflight() {
          Ok(None) => {}
          Ok(Some(_)) => {
            return Err(IntegrationProblem::Store(Message::new(
              "localIntegration.externalResult",
            )));
          }
          Err(_) => {
            return Err(IntegrationProblem::Store(Message::new(
              "localIntegration.preflightFailed",
            )));
          }
        }
      }
      merge_in(&open.repo_root, &programs, (run, expected))
    }));
    let (mut conflicts, mut made) = (Vec::new(), None);
    let merged = match result {
      Ok(Ok(merged)) => {
        running.finish();
        made = Some(merged.merge);
        JobState::Succeeded
      }
      Ok(Err(problem)) => {
        let raw = problem.message(&separator);
        let message = match &policy {
          Some(policy) => policy.safe_message(raw),
          None => raw,
        };
        match &problem {
          IntegrationProblem::Merge(MergeError::Conflict { files }) => conflicts.clone_from(files),
          IntegrationProblem::NotRecorded { merge, .. } => made = Some(merge.clone()),
          _ => {}
        }
        if policy.is_some() {
          running.fail(message.clone());
        } else {
          problem.end(running, not_done);
        }
        JobState::Failed(message)
      }
      Err(_) => {
        tracing::error!("run merge thread panicked");
        running.fail(messages::interrupted());
        JobState::Failed(messages::interrupted())
      }
    };
    if let Some(policy) = &policy {
      conflicts = policy.safe_paths(conflicts);
    }
    self.update(|state| {
      state.merge = merged;
      state.conflicts = conflicts;
      state.merged = made;
    });
    self.finish();
    changed();
  }

  /// Removes what `args` lists on a thread of its own. A person's removal shows in the run's dialog
  /// and row: [`Remover::Person`]'s hook closes the dialog when it is done, and a failure stays under
  /// the row and in the status bar. A removal automation asked for ([`Remover::Automation`]) answers
  /// only its caller: the person's dialog stays open and shows the worktree removed, and a failure
  /// touches neither what the person sees nor the status bar's failures.
  pub fn start_remove(&self, args: RemoveArgs) {
    if !self.begin(Active::Remove) {
      return;
    }
    let what = operations::run_cleanup(&args.open, args.task, (&args.key, args.number));
    self.name(&args.open, &what, false);
    let automation = match &args.by {
      Remover::Person(_) => {
        self.update(|state| state.remove = JobState::Running);
        None
      }
      Remover::Automation(answer) => Some(answer.clone()),
    };
    let job = self.clone();
    if let Err(error) = thread::Builder::new()
      .name("run-cleanup".into())
      .spawn(move || job.remove_now(args))
    {
      tracing::error!(%error, "cannot start run cleanup thread");
      match automation {
        Some(answer) => answer_automation(&answer, Err(messages::interrupted())),
        None => self.update(|state| state.remove = JobState::Failed(messages::interrupted())),
      }
      self.finish();
    }
  }

  fn remove_now(&self, args: RemoveArgs) {
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
      by,
    } = args;
    let running = activity.start(operations::run_cleanup(&open, task, (&key, number)));
    let not_done = Message::new("statusBar.operations.runCleanup.notDone")
      .with("task", key)
      .with("run", number.to_string());
    let result = catch_unwind(AssertUnwindSafe(|| {
      remove_in(&open.repo_root, &programs, &run, &confirmed)
    }));
    let outcome = match (result, &by) {
      (Ok(Ok(())), _) => {
        running.finish();
        Ok(())
      }
      (Ok(Err(problem)), Remover::Person(_)) => {
        let message = problem.message(&separator);
        problem.end(running, not_done);
        Err(message)
      }
      // A tool's failure is only in its result: the operation leaves the status bar without an event.
      (Ok(Err(problem)), Remover::Automation(_)) => {
        drop(running);
        Err(problem.message(&separator))
      }
      (Err(_), remover) => {
        tracing::error!(
          automation = matches!(remover, Remover::Automation(_)),
          "run cleanup thread panicked"
        );
        match remover {
          Remover::Person(_) => running.fail(messages::interrupted()),
          Remover::Automation(_) => drop(running),
        }
        Err(messages::interrupted())
      }
    };
    self.publish_removal(&outcome, &by);
    self.finish();
    changed();
    if let Remover::Automation(answer) = by {
      answer_automation(&answer, outcome);
    }
  }

  fn publish_removal(&self, outcome: &Result<(), Message>, by: &Remover) {
    match (outcome, by) {
      (Ok(()), Remover::Person(removed)) => {
        removed();
        self.update(|state| state.remove = JobState::Succeeded);
      }
      // The person's dialog, if open, now shows the worktree removed.
      (Ok(()), Remover::Automation(_)) => self.update(|state| state.remove = JobState::Succeeded),
      (Err(message), Remover::Person(_)) => self.update(|state| state.remove = JobState::Failed(message.clone())),
      (Err(_), Remover::Automation(_)) => {}
    }
  }

  /// Forgets a removal that failed, which stayed to be shown under the run's row: the person asks
  /// for the removal anew.
  pub fn clear_failed_removal(&self) {
    self
      .jobs
      .0
      .send_if_modified(|state| match state.runs.get_mut(&self.run) {
        Some(run) if matches!(run.remove, JobState::Failed(_)) => {
          run.remove = JobState::Idle;
          true
        }
        _ => false,
      });
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
  pub by: Remover,
}

/// Tells the automation that asked for a removal how it ended. A caller that stopped waiting no longer
/// needs it: the removal stands either way, as the status bar and the run's page show.
fn answer_automation(answer: &mpsc::Sender<Result<(), Message>>, outcome: Result<(), Message>) {
  if answer.send(outcome).is_err() {
    tracing::debug!("the automation that asked for a worktree removal stopped waiting for it");
  }
}

/// Who asked for a removal.
#[derive(Clone)]
pub enum Remover {
  /// A person, in the run's dialog; the hook runs once the worktrees are gone, before the removal
  /// is published as done, and closes that dialog.
  Person(Arc<dyn Fn() + Send + Sync>),
  /// Automation through Orchester's MCP server, which waits for the outcome on this channel.
  Automation(mpsc::Sender<Result<(), Message>>),
}
