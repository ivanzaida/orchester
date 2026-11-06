//! What the attempt page's buttons do besides the task page's merge (`super::super::not_merged::try_again`): *Try
//! again* on a run the merge queue stopped puts it back in its place there (`merge_queue::retry_stopped`); *Try now*
//! and *Stop retrying* end Orchester's wait for its next try (`merge_retry`); *Record merge* records the merge Git made
//! (`merge_record`); *Put files back* puts back what a stopped bring-in left changed (`merge_left`). Each runs off the
//! UI thread; the page is read again after it, and one that did not happen says why on the page. *Open folder* and
//! *Runner log folder* open a folder in the system's file manager. The task page's top card and header press the same
//! work when a merge of the run did not land (`screens::task::merge`).

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use lurq::{app::component::DevtoolsInspectable, core::Signal};
use orchester_core::{id::RunId, time::Timestamp};
use orchester_exec::programs::Programs;
use orchester_workspace::{
  merge_queue,
  messages::{self, Message},
  tasks::{merge_left, merge_record, merge_retry, worktree_removal::WorktreeRemover},
};

/// The page's own work under way, if any.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Busy {
  #[default]
  Idle,
  PuttingBack,
  Recording,
  Requeueing,
}

impl DevtoolsInspectable for Busy {}

/// The buttons' work for run `run` of the project at `root`, whose run worktrees are `worktrees`.
#[derive(Clone)]
pub(crate) struct Actions {
  root: Arc<PathBuf>,
  worktrees: WorktreeRemover,
  programs: Programs,
  run: RunId,
  /// How a list of a failure's causes is joined.
  separator: Arc<str>,
  /// Why the last change asked for did not happen.
  pub failed: Signal<Option<Message>>,
  pub busy: Signal<Busy>,
  /// Counts what makes the page be read again.
  again: Signal<u64>,
}

impl Actions {
  pub(crate) fn new(
    (worktrees, programs, run): (WorktreeRemover, Programs, RunId),
    separator: Arc<str>,
    (failed, busy, again): (Signal<Option<Message>>, Signal<Busy>, Signal<u64>),
  ) -> Self {
    Self {
      root: Arc::new(worktrees.root().to_path_buf()),
      worktrees,
      programs,
      run,
      separator,
      failed,
      busy,
      again,
    }
  }

  /// *Try now*: Orchester's next try begins at once.
  pub(super) fn try_now(&self) {
    self.tell(merge_retry::try_now(self.run), "Try now");
  }

  /// *Stop retrying*: Orchester tries no more; the run stays Not merged as its last attempt left it.
  pub(super) fn stop_retrying(&self) {
    self.tell(merge_retry::stop_retrying(self.run), "Stop retrying");
  }

  fn tell(&self, waited: bool, what: &str) {
    self.failed.set(None);
    if !waited {
      // The wait ended meanwhile: the try went on, or the run is Not merged as its last attempt left it.
      tracing::info!(run = %self.run, what, "found no wait for the run's merge");
    }
    self.again.update(|count| *count += 1);
  }

  /// *Try again* on a run the merge queue stopped: back in its place in the queue.
  pub(crate) fn requeue(&self) {
    let run = self.run;
    self.work(Busy::Requeueing, "merge-attempt-requeue", move |root, _, _| {
      merge_queue::retry_stopped(root, run, Timestamp::now()).map_err(|refusal| refusal.message())
    });
  }

  /// *Record merge*: the merge Git made is recorded; nothing is merged again.
  pub(crate) fn record_merge(&self) {
    let (run, worktrees) = (self.run, self.worktrees.clone());
    self.work(
      Busy::Recording,
      "merge-attempt-record",
      move |_, programs, separator| {
        merge_record::record_merge(&worktrees, programs, run)
          .map(drop)
          .map_err(|problem| problem.message(separator))
      },
    );
  }

  /// *Put files back*.
  pub(crate) fn put_files_back(&self) {
    let run = self.run;
    self.work(
      Busy::PuttingBack,
      "merge-attempt-put-back",
      move |root, programs, separator| {
        merge_left::put_files_back(root, programs, run).map_err(|problem| problem.message(separator))
      },
    );
  }

  /// Does `work` on a thread named `name`, `busy` meanwhile; why it did not happen goes to the page.
  fn work(
    &self,
    busy: Busy,
    name: &str,
    work: impl FnOnce(&Path, &Programs, &str) -> Result<(), Message> + Send + 'static,
  ) {
    let this = self.clone();
    self.failed.set(None);
    self.busy.set(busy);
    let started = std::thread::Builder::new().name(name.to_owned()).spawn(move || {
      if let Err(message) = work(&this.root, &this.programs, &this.separator) {
        this.failed.set(Some(message));
      }
      this.busy.set(Busy::Idle);
      this.again.update(|count| *count += 1);
    });
    if let Err(error) = started {
      tracing::error!(%error, name, "cannot start the attempt page's work");
      self.busy.set(Busy::Idle);
      self.failed.set(Some(messages::interrupted()));
    }
  }
}

/// Opens `folder` in the system's file manager on a thread named `name`, making it first when it is not there yet (the
/// runner writes its log folder once it logs).
pub(super) fn open_folder(name: &str, folder: &Path) {
  let folder = folder.to_path_buf();
  let opened = std::thread::Builder::new().name(name.to_owned()).spawn(move || {
    if let Err(error) = std::fs::create_dir_all(&folder) {
      tracing::warn!(%error, folder = %folder.display(), "cannot make the folder to open");
    }
    if let Err(error) = orchester_exec::external::open_path(&folder) {
      tracing::warn!(%error, folder = %folder.display(), "the system did not open the folder");
    }
  });
  if let Err(error) = opened {
    tracing::error!(%error, "cannot start opening a folder");
  }
}
