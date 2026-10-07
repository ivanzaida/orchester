//! One project's merge and worktree-removal operations, shared by all of its task pages and tabs.
//! Git work continues after a page unmounts; its state and result must have the same lifetime.

use std::{
  collections::HashMap,
  panic::{AssertUnwindSafe, catch_unwind},
  sync::{Arc, mpsc},
  thread,
};

use lurq::core::Signal;
use orchester_core::{
  id::{ProjectId, RunId},
  integration::Leftovers,
};
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

  /// Removes what `args` lists on a thread of its own. A person's removal shows in the run's dialog
  /// and row: it closes the dialog when it is done, and a failure stays under the row and in the status
  /// bar. A removal automation asked for ([`Remover::Automation`]) answers only its caller: the
  /// person's dialog stays open and shows the worktree removed, and a failure touches neither what
  /// the person sees nor the status bar's failures.
  pub fn start_remove(&self, args: RemoveArgs) {
    if !self.begin(Active::Remove) {
      return;
    }
    let automation = match &args.by {
      Remover::Person => {
        self.0.remove.set(JobState::Running);
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
        None => self.0.remove.set(JobState::Failed(messages::interrupted())),
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
      (Ok(Err(problem)), Remover::Person) => {
        let message = problem.message(&separator);
        problem.end(running, not_done);
        Err(message)
      }
      (Ok(Err(problem)), Remover::Automation(_)) => {
        running.finish_as(not_done);
        Err(problem.message(&separator))
      }
      (Err(_), remover) => {
        tracing::error!("run cleanup thread panicked");
        match remover {
          Remover::Person => running.fail(messages::interrupted()),
          Remover::Automation(_) => running.finish_as(not_done),
        }
        Err(messages::interrupted())
      }
    };
    match (&outcome, &by) {
      (Ok(()), Remover::Person) => {
        self.0.dialog_open.set(false);
        self.0.remove.set(JobState::Succeeded);
      }
      // The person's dialog, if open, now shows the worktree removed.
      (Ok(()), Remover::Automation(_)) => self.0.remove.set(JobState::Succeeded),
      (Err(message), Remover::Person) => self.0.remove.set(JobState::Failed(message.clone())),
      (Err(_), Remover::Automation(_)) => {}
    }
    self.finish();
    changed();
    if let Remover::Automation(answer) = by {
      answer_automation(&answer, outcome);
    }
  }

  /// Opens the confirmation of *Remove worktree* anew: discarding not agreed to, and an earlier
  /// failure, which stayed under the run's row once the dialog closed, cleared.
  pub fn open_remove(&self) {
    if matches!(self.0.remove.get_untracked(), JobState::Failed(_)) {
      self.0.remove.set(JobState::Idle);
    }
    self.0.discard.set(false);
    self.0.dialog_open.set(true);
  }

  /// Closes the confirmation unless git is removing; a failure stays to be shown under the run's
  /// row in *Runs of this task* (63.06 `VBR27`).
  pub fn dismiss_remove(&self) {
    if self.0.remove.get_untracked() == JobState::Running {
      return;
    }
    self.0.dialog_open.set(false);
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
  /// A person, in the run's dialog.
  Person,
  /// Automation through Orchester's MCP server, which waits for the outcome on this channel.
  Automation(mpsc::Sender<Result<(), Message>>),
}

/// Project-scoped registry retained while any tab still names the project.
#[derive(Clone)]
pub struct IntegrationJobs(Arc<Mutex<HashMap<RunId, RunIntegration>>>, Signal<u64>);

impl Default for IntegrationJobs {
  fn default() -> Self {
    Self(Arc::default(), Signal::new(0))
  }
}

/// Every project's merge and removal operations, shared by the window's tabs and Orchester's MCP server,
/// so a removal a tool starts guards the task page's buttons and the other way round.
#[derive(Clone, Default)]
pub struct ProjectJobs(Arc<Mutex<HashMap<ProjectId, IntegrationJobs>>>);

impl PartialEq for ProjectJobs {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl lurq::app::component::DevtoolsInspectable for ProjectJobs {}

impl ProjectJobs {
  /// The operations of `project`, made the first time they are asked for.
  pub fn of(&self, project: ProjectId) -> IntegrationJobs {
    self.0.lock().entry(project).or_default().clone()
  }
}

impl IntegrationJobs {
  /// Counts the removals Orchester's MCP server made: the task pages read what the runs left again.
  pub fn removed_outside(&self) -> Signal<u64> {
    self.1.clone()
  }

  pub fn of(&self, run: RunId) -> RunIntegration {
    self.0.lock().entry(run).or_insert_with(RunIntegration::new).clone()
  }
}
