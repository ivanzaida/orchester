//! The run's changes as the task page reads them, once for both the Changes tab and the count on its
//! label, so the two cannot disagree: the diff from the run's base to its latest checkpoint, read again
//! when it records a new one, and what its worktree holds beyond that, read again after a checkpoint,
//! when the run ends, when the tab opens or closes and, while an agent works and the page is on screen,
//! every few seconds. What was read for one run is never shown for another, and a read again after a
//! failure shows as being read.

use std::{path::Path, sync::Arc, time::Duration};

use lurq::{
  app::ctx::{Ctx, FutureState, StreamEmitter},
  core::Signal,
};
use orchester_core::{id::RunId, time::Timestamp};
use orchester_workspace::{
  background,
  messages::Message,
  tasks::{
    changes::{Checkpointed, NotCheckpointed, Refresh, checkpointed_in, listed_files, not_checkpointed_in},
    detail::RunView,
  },
};
use parking_lot::Mutex;

use crate::services::Services;

/// How often the worktree is read again while an agent works in it.
const WORKTREE_REFRESH: Duration = Duration::from_secs(3);

/// The run's checkpointed changes as read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum CheckpointedRead {
  #[default]
  Loading,
  Ready(Checkpointed),
  Failed(Message),
}

impl lurq::app::component::DevtoolsInspectable for CheckpointedRead {}

/// The latest run's changes as the task page read them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunChanges {
  pub checkpointed: CheckpointedRead,
  /// What the run's worktree holds beyond its checkpoint; `None` until read.
  pub worktree: Option<Result<NotCheckpointed, Message>>,
}

impl lurq::app::component::DevtoolsInspectable for RunChanges {}

impl RunChanges {
  /// How many files the Changes tab lists, the count on its label; `None` while that is not known
  /// ([`listed_files`]).
  pub fn listed_files(&self) -> Option<usize> {
    let checkpointed = match &self.checkpointed {
      CheckpointedRead::Loading => None,
      CheckpointedRead::Ready(checkpointed) => Some(Ok(checkpointed)),
      CheckpointedRead::Failed(error) => Some(Err(error)),
    };
    listed_files(checkpointed, self.worktree.as_ref().map(Result::as_ref))
  }
}

/// When the worktree was last read; shared by the reads and the line that says how long ago that was.
#[derive(Clone, Default)]
pub struct LastRead(Arc<Mutex<Option<Timestamp>>>);

impl PartialEq for LastRead {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl LastRead {
  pub fn get(&self) -> Option<Timestamp> {
    *self.0.lock()
  }

  fn set(&self, at: Timestamp) {
    *self.0.lock() = Some(at);
  }
}

/// When the worktree is read: again and again while an agent works on the run, and once more when the
/// Changes tab opens or closes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Follow {
  pub agent_at_work: bool,
  pub tab_open: bool,
}

/// The reads of one task page: it creates them once, in its `create`, and reads with them on every render
/// that has a run. The worktree is read again on each tick of an interval of the page's own, which lurq
/// does not run while the page's window tab is offstage.
#[derive(Clone)]
pub struct ChangesReads {
  refresh: Arc<Refresh>,
  /// Counts Try again: the changes are read again after they could not be read.
  retry: Signal<u64>,
  last_read: LastRead,
}

impl ChangesReads {
  pub fn new(ctx: &mut Ctx) -> Self {
    let refresh = Refresh::default();
    ctx.create_interval(WORKTREE_REFRESH, refresh.ticker()).start();
    Self {
      refresh: Arc::new(refresh),
      retry: ctx.signal(0),
      last_read: LastRead::default(),
    }
  }

  /// The changes of `run` in the repository at `repo_root`.
  pub fn read(&self, ctx: &mut Ctx, repo_root: &Path, run: &RunView, follow: Follow) -> RunChanges {
    let retry = self.retry.get();
    RunChanges {
      checkpointed: self.checkpointed(ctx, repo_root, run, retry),
      worktree: self.not_checkpointed(ctx, repo_root, run, (follow, retry)),
    }
  }

  pub(super) fn retry(&self) {
    self.retry.update(|attempt| *attempt += 1);
  }

  pub(super) fn last_read(&self) -> LastRead {
    self.last_read.clone()
  }

  fn checkpointed(&self, ctx: &mut Ctx, repo_root: &Path, run: &RunView, retry: u64) -> CheckpointedRead {
    let programs = Services::of(ctx).programs;
    let head = run
      .run
      .last_checkpoint()
      .map(|checkpoint| checkpoint.revision().clone());
    let deps = (repo_root.to_path_buf(), run.id(), run.run.base().clone(), head, retry);
    let load = ctx.future(deps, move |(repo_root, run, base, head, _)| {
      let programs = programs.clone();
      async move {
        let read = background::run(move || checkpointed_in(&repo_root, &programs, &base, head.as_ref()))
          .await
          .and_then(|read| {
            read.map_err(|error| {
              tracing::warn!(%error, "cannot read the run's changes");
              error.message()
            })
          });
        Ok::<_, ()>(ForRun { run, read })
      }
    });
    match current(load.state().get(), run.id()) {
      None => CheckpointedRead::Loading,
      Some(Ok(checkpointed)) => CheckpointedRead::Ready(checkpointed),
      Some(Err(error)) => CheckpointedRead::Failed(error),
    }
  }

  /// What the worktree of `run` holds beyond its commit; `None` until read. Read again on each tick of
  /// the refresh while followed; the page renders again only when what was read changed.
  fn not_checkpointed(
    &self,
    ctx: &mut Ctx,
    repo_root: &Path,
    run: &RunView,
    (follow, retry): (Follow, u64),
  ) -> Option<Result<NotCheckpointed, Message>> {
    let programs = Services::of(ctx).programs;
    let (follower, last_read) = (self.refresh.follower(), self.last_read.clone());
    let head = run
      .run
      .last_checkpoint()
      .map(|checkpoint| checkpoint.revision().clone());
    let read_at = (run.id(), run.run.status().is_terminal());
    let deps = (repo_root.to_path_buf(), read_at, head, follow, retry);
    let reads = ctx.stream(
      deps,
      move |(repo_root, read_at, _, follow, _), emitter: StreamEmitter<ForRun<NotCheckpointed>, ()>| {
        let (programs, follower, last_read) = (programs.clone(), follower.clone(), last_read.clone());
        let read = move || {
          let (repo_root, programs, last_read) = (repo_root.clone(), programs.clone(), last_read.clone());
          async move {
            let read = background::run(move || not_checkpointed_in(&repo_root, &programs, read_at))
              .await
              .and_then(|read| {
                read.map_err(|error| {
                  tracing::warn!(%error, "cannot read the run's worktree");
                  error.message()
                })
              });
            last_read.set(Timestamp::now());
            ForRun { run: read_at.0, read }
          }
        };
        follower.follow(follow.agent_at_work, read, move |read| emitter.emit(read))
      },
    );
    current(reads.state().get(), run.id())
  }
}

/// A read of `run`'s changes.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ForRun<T> {
  run: RunId,
  read: Result<T, Message>,
}

impl<T> lurq::app::component::DevtoolsInspectable for ForRun<T> {}

/// What was read for `run`: `None` before its first read, while what is there was read for another run,
/// and while it is read again after it failed.
fn current<T>(state: FutureState<ForRun<T>, ()>, run: RunId) -> Option<Result<T, Message>> {
  let pending = state.is_pending();
  let data = state.data.filter(|data| data.run == run)?;
  if pending && data.read.is_err() {
    return None;
  }
  Some(data.read)
}
