//! The run's changes as the task page reads them, once for both the Changes tab and the count on its
//! label, so the two cannot disagree: the diff from where the run's own work starts (its base, or the newer commit
//! of its base branch its branch took in) to its latest checkpoint, read again
//! when it records a new one, and what its worktree holds beyond that, read again after a checkpoint,
//! when the run ends, when the tab opens or closes and, while an agent works and the page is on screen,
//! every few seconds. What was read for one run is never shown for another, and a read again after a
//! failure shows as being read.

use std::{path::Path, sync::Arc, time::Duration};

use lurq::{
  app::ctx::{Ctx, FutureState, StreamEmitter},
  core::Signal,
};
use orchester_core::{id::RunId, run::TaskPhase, time::Timestamp};
use orchester_workspace::{
  background,
  messages::Message,
  tasks::{
    changes::{
      Checkpointed, NotCheckpointed, Refresh, RunRange, RunStage, listed_files, not_checkpointed_in, run_changes_in,
    },
    detail::{RunView, TaskDetail},
  },
};
use parking_lot::Mutex;

use crate::{
  routes::{ChangedFile, TaskView},
  services::Services,
  ui::tabs::counted,
};

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

/// The count on the Changes tab's label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileCount {
  pub files: usize,
  /// The worktree's part is known; otherwise `files` counts the checkpointed files only.
  pub complete: bool,
}

impl RunChanges {
  /// How many files the Changes tab lists ([`listed_files`]); when only the worktree's part is not known
  /// (too large to list, or unreadable), the checkpointed files, marked as not all. `None` while reading,
  /// and when the checkpointed part is not known either.
  pub fn file_count(&self) -> Option<FileCount> {
    let checkpointed = match &self.checkpointed {
      CheckpointedRead::Loading => None,
      CheckpointedRead::Ready(checkpointed) => Some(Ok(checkpointed)),
      CheckpointedRead::Failed(error) => Some(Err(error)),
    };
    let worktree = self.worktree.as_ref().map(Result::as_ref);
    if let Some(files) = listed_files(checkpointed, worktree) {
      return Some(FileCount { files, complete: true });
    }
    let CheckpointedRead::Ready(checkpointed) = &self.checkpointed else {
      return None;
    };
    match worktree? {
      Err(_) | Ok(NotCheckpointed::TooLarge { .. }) => {
        checkpointed.files().map(|files| FileCount { files, complete: false })
      }
      Ok(NotCheckpointed::Clean | NotCheckpointed::Removed | NotCheckpointed::Read(_)) => None,
    }
  }
}

/// The Changes tab's label `name` with `count` when it is known: `Changes 6`, or `Changes 4+` while the
/// worktree's files are not known.
pub fn tab_label(ctx: &Ctx, name: &str, count: Option<FileCount>) -> Arc<str> {
  match count {
    None => name.into(),
    Some(FileCount { files, complete: true }) => counted(ctx, name, files),
    Some(FileCount { files, complete: false }) => ctx.t_args(
      "changes.tabCountPartial",
      [("label", name.to_owned()), ("count", files.to_string())],
    ),
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

impl Follow {
  /// For `detail`'s page while it shows `view`, while its project's runner is `online` or not: an agent works on the
  /// run while it runs and its runner is there. Local integration, in the Changes tab's place, shows none of its files.
  pub fn of(detail: &TaskDetail, online: bool, view: &TaskView) -> Self {
    Self {
      agent_at_work: detail.phase() == TaskPhase::Running && !detail.lost_its_runner(online),
      tab_open: matches!(view, TaskView::Changes(_)),
    }
  }
}

/// The reads of one task page: it creates them once, in its `create`, and provides them to its parts
/// (`ctx.provide`); the part that shows the tab's count reads with them on every render
/// ([`ChangesReads::read`]) and the Changes tab shows what was read last ([`ChangesReads::latest`]), so
/// the two never disagree and nothing is read twice. The worktree is read again on each tick of an
/// interval of the page's own, which lurq does not run while the page's window tab is offstage.
#[derive(Clone)]
pub struct ChangesReads {
  refresh: Arc<Refresh>,
  /// Counts Try again: the changes are read again after they could not be read.
  retry: Signal<u64>,
  last_read: LastRead,
  /// What was read last; `None` without a run.
  latest: Signal<Option<RunChanges>>,
  /// The file the Changes tab showed last: it stays shown while no file is named and it is listed, and
  /// the tab opens on it again.
  shown: Arc<Mutex<Option<ChangedFile>>>,
}

impl ChangesReads {
  pub fn new(ctx: &mut Ctx) -> Self {
    let refresh = Refresh::default();
    ctx.create_interval(WORKTREE_REFRESH, refresh.ticker()).start();
    Self {
      refresh: Arc::new(refresh),
      retry: ctx.signal(0),
      last_read: LastRead::default(),
      latest: ctx.signal(None),
      shown: Arc::default(),
    }
  }

  /// The reads the task page provides to its parts.
  pub fn of(ctx: &mut Ctx) -> Self {
    ctx
      .use_context::<Self>()
      .expect("the task page provides its ChangesReads before mounting its parts")
  }

  /// What was read last; the caller renders again when it changes.
  pub(super) fn latest(&self) -> Option<RunChanges> {
    self.latest.get()
  }

  /// The changes of `detail`'s latest run in the repository at `repo_root`; `None` without a run. Call it
  /// on every render of the page, with or without a run: its reads keep their places among the page's.
  pub fn read(&self, ctx: &mut Ctx, repo_root: &Path, detail: &TaskDetail, follow: Follow) -> Option<RunChanges> {
    let retry = self.retry.get();
    let run = detail.run.as_ref();
    let checkpointed = self.checkpointed(ctx, repo_root, run, retry);
    let worktree = self.not_checkpointed(ctx, repo_root, run, (follow, retry));
    let changes = run.map(|_| RunChanges { checkpointed, worktree });
    if self.latest.get_untracked() != changes {
      self.latest.set(changes.clone());
    }
    changes
  }

  /// The file the Changes tab showed last, for the page tabs to open it on.
  pub fn shown_file(&self) -> Option<ChangedFile> {
    self.shown.lock().clone()
  }

  pub(super) fn set_shown(&self, file: ChangedFile) {
    *self.shown.lock() = Some(file);
  }

  pub(super) fn retry(&self) {
    self.retry.update(|attempt| *attempt += 1);
  }

  pub(super) fn last_read(&self) -> LastRead {
    self.last_read.clone()
  }

  fn checkpointed(&self, ctx: &mut Ctx, repo_root: &Path, run: Option<&RunView>, retry: u64) -> CheckpointedRead {
    let programs = Services::of(ctx).programs;
    let range = run.map(|run| RunRange::of(&run.run));
    let deps = (repo_root.to_path_buf(), range, retry);
    let load = ctx.future(deps, move |(repo_root, range, _)| {
      let programs = programs.clone();
      async move {
        let Some(range) = range else {
          return Ok::<_, ()>(None);
        };
        let run = range.run;
        let read = background::run(move || run_changes_in(&repo_root, &programs, &range))
          .await
          .and_then(|read| {
            read.map_err(|error| {
              tracing::warn!(%error, "cannot read the run's changes");
              error.message()
            })
          });
        Ok(Some(ForRun { run, read }))
      }
    });
    match run.and_then(|run| current(load.state().get(), run.id())) {
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
    run: Option<&RunView>,
    (follow, retry): (Follow, u64),
  ) -> Option<Result<NotCheckpointed, Message>> {
    let programs = Services::of(ctx).programs;
    let (follower, last_read) = (self.refresh.follower(), self.last_read.clone());
    let at = run.map(|run| {
      let head = run
        .run
        .last_checkpoint()
        .map(|checkpoint| checkpoint.revision().clone());
      ((run.id(), RunStage::of(run.run.status())), head)
    });
    let deps = (repo_root.to_path_buf(), at, follow, retry);
    let reads = ctx.stream(
      deps,
      move |(repo_root, at, follow, _), emitter: StreamEmitter<Option<ForRun<NotCheckpointed>>, ()>| {
        let (programs, follower, last_read) = (programs.clone(), follower.clone(), last_read.clone());
        async move {
          let Some((read_at, _)) = at else {
            emitter.emit(None);
            return;
          };
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
          follower
            .follow(follow.agent_at_work, read, move |read| emitter.emit(Some(read)))
            .await;
        }
      },
    );
    run.and_then(|run| current(reads.state().get(), run.id()))
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
fn current<T>(state: FutureState<Option<ForRun<T>>, ()>, run: RunId) -> Option<Result<T, Message>> {
  let pending = state.is_pending();
  let data = state.data.flatten().filter(|data| data.run == run)?;
  if pending && data.read.is_err() {
    return None;
  }
  Some(data.read)
}
