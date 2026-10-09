//! The run's changes as the task page reads them, once for both the Changes tab and the count on
//! its label, so the two cannot disagree: the diff from the run's base to its latest checkpoint,
//! read again when it records a new one, and what its worktree holds beyond that, read again
//! after a checkpoint, when the run ends, when the tab opens or closes and, while an agent works
//! and the page is on screen, every few seconds. What was read for one run is never shown for
//! another, and a read again after a failure shows as being read.

use std::{path::Path, time::Duration};

use lurq::app::ctx::{Ctx, FutureState, StreamEmitter};
use orchester_core::id::RunId;

use super::CheckpointedLoad;
use crate::{
  background,
  messages::Message,
  programs::Programs,
  tasks::{
    changes::{NotCheckpointed, Refresh, RunStage, checkpointed_in, listed_files, not_checkpointed_in},
    detail::RunView,
  },
};

/// How often the worktree is read again while an agent works in it.
const WORKTREE_REFRESH: Duration = Duration::from_secs(3);

/// A read of `run`'s changes.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ForRun<T> {
  run: RunId,
  read: Result<T, Message>,
}

impl<T> lurq::app::component::DevtoolsInspectable for ForRun<T> {}

/// What was read for `run`: `None` before its first read, while what is there was read for
/// another run, and while it is read again after it failed.
fn current<T>(state: FutureState<ForRun<T>, ()>, run: RunId) -> Option<Result<T, Message>> {
  let pending = state.is_pending();
  let data = state.data.filter(|data| data.run == run)?;
  if pending && data.read.is_err() {
    return None;
  }
  Some(data.read)
}

/// The page's [`Refresh`] of the worktree reads, ticked by an interval of the page's own, which
/// lurq does not run while the page's window tab is offstage.
pub(in super::super) fn worktree_refresh(ctx: &mut Ctx) -> Refresh {
  let refresh = Refresh::default();
  ctx.create_interval(WORKTREE_REFRESH, refresh.ticker()).start();
  refresh
}

/// The changes of `run` from its base to its latest checkpoint; `retry` reads them again after
/// they could not be read.
pub(in super::super) fn checkpointed(ctx: &mut Ctx, repo_root: &Path, run: &RunView, retry: u64) -> CheckpointedLoad {
  let programs = ctx
    .use_context::<Programs>()
    .expect("the root provides Programs before mounting screens");
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
    None => CheckpointedLoad::Loading,
    Some(Ok(checkpointed)) => CheckpointedLoad::Ready(checkpointed),
    Some(Err(error)) => CheckpointedLoad::Failed(error),
  }
}

/// What the worktree of `run` holds beyond its commit; `None` until read. Read again on each tick
/// of `refresh` while `live`, an agent at work on the run; the page renders again only when what
/// was read changed.
pub(in super::super) fn not_checkpointed(
  ctx: &mut Ctx,
  repo_root: &Path,
  run: &RunView,
  refresh: &Refresh,
  (live, tab_open): (bool, bool),
) -> Option<Result<NotCheckpointed, Message>> {
  let programs = ctx
    .use_context::<Programs>()
    .expect("the root provides Programs before mounting screens");
  let follower = refresh.follower();
  let head = run
    .run
    .last_checkpoint()
    .map(|checkpoint| checkpoint.revision().clone());
  let read_at = (run.id(), RunStage::of(run.run.status()));
  let deps = (repo_root.to_path_buf(), read_at, head, live, tab_open);
  let reads = ctx.stream(
    deps,
    move |(repo_root, read_at, _, live, _), emitter: StreamEmitter<ForRun<NotCheckpointed>, ()>| {
      let (programs, follower) = (programs.clone(), follower.clone());
      let read = move || {
        let (repo_root, programs) = (repo_root.clone(), programs.clone());
        async move {
          let read = background::run(move || not_checkpointed_in(&repo_root, &programs, read_at))
            .await
            .and_then(|read| {
              read.map_err(|error| {
                tracing::warn!(%error, "cannot read the run's worktree");
                error.message()
              })
            });
          ForRun { run: read_at.0, read }
        }
      };
      follower.follow(live, read, move |read| emitter.emit(read))
    },
  );
  current(reads.state().get(), run.id())
}

/// The latest run's changes as the task page read them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in super::super) struct RunChanges {
  pub(in super::super) checkpointed: CheckpointedLoad,
  /// What the run's worktree holds beyond its checkpoint; `None` until read.
  pub(in super::super) worktree: Option<Result<NotCheckpointed, Message>>,
}

impl RunChanges {
  /// How many files the Changes tab lists ([`listed_files`]).
  pub(in super::super) fn listed_files(&self) -> Option<usize> {
    let checkpointed = match &self.checkpointed {
      CheckpointedLoad::Loading => None,
      CheckpointedLoad::Ready(checkpointed) => Some(Ok(checkpointed)),
      CheckpointedLoad::Failed(error) => Some(Err(error)),
    };
    listed_files(checkpointed, self.worktree.as_ref().map(Result::as_ref))
  }
}
