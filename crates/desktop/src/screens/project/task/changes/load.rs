//! The run's changes as the task page reads them, once for both the Changes tab and the count on
//! its label, so the two cannot disagree: the diff from the run's base to its latest checkpoint,
//! read again when it records a new one, and what its worktree holds beyond that, read again
//! after a checkpoint, when the run ends, when the tab opens or closes and, while an agent works,
//! every few seconds.

use std::{collections::HashSet, path::Path, time::Duration};

use lurq::app::ctx::{Ctx, StreamEmitter};
use orchester_core::diff::FileDiff;

use super::CheckpointedLoad;
use crate::{
  background,
  messages::Message,
  programs::Programs,
  tasks::{
    changes::{Checkpointed, NotCheckpointed, checkpointed_in, not_checkpointed_in},
    detail::RunView,
  },
};

/// How often the worktree is read again while an agent works in it.
const WORKTREE_REFRESH: Duration = Duration::from_secs(3);

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
  let deps = (repo_root.to_path_buf(), run.run.base().clone(), head, retry);
  let load = ctx.future(deps, move |(repo_root, base, head, _)| {
    let programs = programs.clone();
    async move {
      background::run(move || checkpointed_in(&repo_root, &programs, &base, head.as_ref()))
        .await?
        .map_err(|error| {
          tracing::warn!(%error, "cannot read the run's changes");
          error.message()
        })
    }
  });
  let state = load.state().get();
  match (state.data, state.error) {
    (_, Some(error)) => CheckpointedLoad::Failed(error),
    (Some(checkpointed), None) => CheckpointedLoad::Ready(checkpointed),
    (None, None) => CheckpointedLoad::Loading,
  }
}

/// What the worktree of `run` holds beyond its commit; `None` until first read. Read again every
/// [`WORKTREE_REFRESH`] while `live`, an agent at work on the run; the page renders again only
/// when what was read changed.
pub(in super::super) fn not_checkpointed(
  ctx: &mut Ctx,
  repo_root: &Path,
  run: &RunView,
  (live, tab_open): (bool, bool),
) -> Option<Result<NotCheckpointed, Message>> {
  let programs = ctx
    .use_context::<Programs>()
    .expect("the root provides Programs before mounting screens");
  let head = run
    .run
    .last_checkpoint()
    .map(|checkpoint| checkpoint.revision().clone());
  let read_at = (run.id(), run.run.status().is_terminal());
  let deps = (repo_root.to_path_buf(), read_at, head, live, tab_open);
  let reads = ctx.stream(
    deps,
    move |(repo_root, read_at, _, live, _), emitter: StreamEmitter<NotCheckpointed, Message>| {
      let programs = programs.clone();
      async move {
        let mut last = None;
        loop {
          let (repo_root, programs) = (repo_root.clone(), programs.clone());
          let read = background::run(move || not_checkpointed_in(&repo_root, &programs, read_at))
            .await
            .and_then(|read| {
              read.map_err(|error| {
                tracing::warn!(%error, "cannot read the run's worktree");
                error.message()
              })
            });
          if last.as_ref() != Some(&read) {
            let sent = match read.clone() {
              Ok(changes) => emitter.emit(changes),
              Err(error) => emitter.reject(error),
            };
            if !sent {
              return;
            }
            last = Some(read);
          }
          if !live {
            return;
          }
          tokio::time::sleep(WORKTREE_REFRESH).await;
        }
      }
    },
  );
  let state = reads.state().get();
  match (state.data, state.error) {
    (_, Some(error)) => Some(Err(error)),
    (Some(changes), None) => Some(Ok(changes)),
    (None, None) => None,
  }
}

/// The latest run's changes as the task page read them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in super::super) struct RunChanges {
  pub(in super::super) checkpointed: CheckpointedLoad,
  /// What the run's worktree holds beyond its checkpoint; `None` until first read.
  pub(in super::super) worktree: Option<Result<NotCheckpointed, Message>>,
}

impl RunChanges {
  /// How many files the Changes tab lists, a file changed both up to the checkpoint and beyond it
  /// counted once. `None` while that is not known: either part is still being read, could not be
  /// read, or is too large to list file by file beside the other; and while the run has neither
  /// committed nor changed anything.
  pub(in super::super) fn listed_files(&self) -> Option<usize> {
    let CheckpointedLoad::Ready(checkpointed) = &self.checkpointed else {
      return None;
    };
    let committed = match checkpointed {
      // The tab shows only the size of such a diff, and nothing of the worktree beside it.
      Checkpointed::TooLarge { .. } => return checkpointed.files(),
      Checkpointed::Read { changes, .. } => Some(changes),
      Checkpointed::NoCheckpoint => None,
    };
    let uncommitted = match self.worktree.as_ref()? {
      Ok(NotCheckpointed::Read(changes)) => Some(changes),
      Ok(NotCheckpointed::Clean | NotCheckpointed::Removed) => None,
      Ok(NotCheckpointed::TooLarge { .. }) | Err(_) => return None,
    };
    if committed.is_none() && uncommitted.is_none() {
      return None;
    }
    let paths: HashSet<&str> = committed
      .into_iter()
      .chain(uncommitted)
      .flat_map(|changes| changes.diff.files().iter().map(FileDiff::path))
      .collect();
    Some(paths.len())
  }
}
