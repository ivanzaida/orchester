//! What the task page reads off the UI thread: the task with its latest run and its runs
//! (`TaskDetail`), and what the task's runs without live work left in the repository (their worktrees
//! and branches), for *Remove worktree* and Run again's removal.

use std::{path::PathBuf, sync::Arc};

use lurq::app::ctx::{Ctx, StreamEmitter};
use orchester_core::{
  id::{RunId, TaskId},
  integration::Leftovers,
  run::Run,
};
use orchester_exec::programs::Programs;
use orchester_workspace::{
  background,
  messages::{self, Message},
  tasks::{
    detail::TaskDetail,
    history::RunRecord,
    integration::{LeftBehind, leftovers_in},
    integration_jobs::{IntegrationJobs, ProjectJobState},
  },
};

/// The task as read, shared by the page's parts.
#[derive(Clone, Debug, PartialEq)]
pub struct LoadedTask(pub Arc<TaskDetail>);

impl lurq::app::component::DevtoolsInspectable for LoadedTask {}

/// How the task's reading stands.
pub enum TaskRead {
  Loading,
  /// The task as last read, and why reading it again failed, if it did.
  Ready(Arc<TaskDetail>, Option<Message>),
  /// The first reading failed, and why.
  Failed(Message),
}

/// What makes the task page read the task again: the runner changed a run or its transcript, a screen
/// or automation saved tasks, or the person asked to try again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReadAgain {
  pub changes: u64,
  pub saves: u64,
  pub retries: u64,
}

/// Reads `task` of the project at `root`, again whenever `again` changes.
pub fn task(ctx: &mut Ctx, root: PathBuf, task: TaskId, again: ReadAgain) -> TaskRead {
  let load = ctx.future((root, task, again), |(root, task, _)| async move {
    background::run(move || TaskDetail::load_in(&root, task))
      .await?
      .map(|detail| LoadedTask(Arc::new(detail)))
      .map_err(|error| {
        tracing::error!(%error, "cannot read the task");
        messages::store_error(&error)
      })
  });
  let state = load.state().get();
  match (state.data, state.error) {
    (Some(LoadedTask(detail)), stale) => TaskRead::Ready(detail, stale),
    (None, Some(error)) => TaskRead::Failed(error),
    (None, None) => TaskRead::Loading,
  }
}

/// What each run left, by run.
type LeftByRun = Arc<[(RunId, Leftovers)]>;

/// What the task's runs without live work left, by run, or why it could not be read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TaskLeftovers {
  left: Option<Result<LeftByRun, Message>>,
  /// Read again after a removal: what was read before must not offer the removal again.
  stale: bool,
}

impl lurq::app::component::DevtoolsInspectable for TaskLeftovers {}

impl TaskLeftovers {
  /// What `run` left, for its actions; `None` until read, while read again, or for a run with live
  /// work, which was not read.
  pub fn of(&self, run: RunId) -> Option<Result<Leftovers, Message>> {
    if self.stale {
      return None;
    }
    self.as_read(run)
  }

  /// What `run` left as last read, for its row, which keeps its line while it is read again.
  pub fn as_read(&self, run: RunId) -> Option<Result<Leftovers, Message>> {
    match self.left.as_ref()? {
      Ok(left) => left
        .iter()
        .find(|(id, _)| *id == run)
        .map(|(_, leftovers)| Ok(leftovers.clone())),
      Err(error) => Some(Err(error.clone())),
    }
  }
}

/// What was read, with what had changed when it was read.
#[derive(Clone, Debug, PartialEq)]
struct Read {
  at: (u64, u64),
  left: LeftByRun,
}

impl lurq::app::component::DevtoolsInspectable for Read {}

/// What the runs of `detail` without live work left, read again when a run ends or starts, when tasks
/// were saved (a removal saves) or when automation removed worktrees (`(saves, removed_outside)`);
/// `separator` joins the names of files a problem lists.
pub fn leftovers(
  ctx: &mut Ctx,
  (root, programs, separator): (PathBuf, Programs, Arc<str>),
  detail: &TaskDetail,
  changed: (u64, u64),
) -> TaskLeftovers {
  let runs: Vec<Arc<Run>> = detail
    .history
    .iter()
    .filter_map(RunRecord::without_live_work)
    .cloned()
    .collect();
  let ids: Vec<RunId> = runs.iter().map(|run| run.id()).collect();
  let deps = (root, ids, detail.history.len(), changed);
  let load = ctx.future(deps, move |(root, _, _, at)| {
    let (programs, runs, separator) = (programs.clone(), runs.clone(), separator.clone());
    async move {
      if runs.is_empty() {
        return Ok(Read {
          at,
          left: Arc::from([]),
        });
      }
      background::run(move || leftovers_in(&root, &programs, &runs))
        .await?
        .map(|LeftBehind(left)| Read { at, left: left.into() })
        .map_err(|problem| problem.message(&separator))
    }
  });
  let state = load.state().get();
  let stale = state.is_pending() && state.data.as_ref().is_some_and(|read| read.at != changed);
  let left = match (state.data, state.error) {
    (_, Some(error)) => Some(Err(error)),
    (Some(read), None) => Some(Ok(read.left)),
    (None, None) => None,
  };
  TaskLeftovers { left, stale }
}

/// The project's merge and removal operations whenever they change: the ones the window and
/// automation share (`ProjectJobs`), so a removal either starts keeps the other's buttons waiting.
pub fn jobs(ctx: &mut Ctx, jobs: IntegrationJobs) -> ProjectJobState {
  let state = ctx.stream(jobs, |jobs, emitter: StreamEmitter<ProjectJobState, ()>| async move {
    let mut updates = jobs.subscribe();
    loop {
      let state = updates.borrow_and_update().clone();
      if !emitter.emit(state) || updates.changed().await.is_err() {
        return;
      }
    }
  });
  state.state().get().data.unwrap_or_default()
}
