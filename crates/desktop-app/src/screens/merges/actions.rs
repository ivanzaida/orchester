//! What Review › Merges' buttons do that is not the task page's merge: *Try now* ends the wait for Orchester's next try
//! (`orchester_workspace::tasks::merge_retry::try_now`), *Put files back* puts back what a stopped bring-in left
//! changed (`merge_left::put_files_back`) off the UI thread. The merges are read again after either; one that did not
//! happen says why on the screen.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use lurq::{app::component::DevtoolsInspectable, core::Signal};
use orchester_core::id::RunId;
use orchester_exec::programs::Programs;
use orchester_workspace::{
  messages::Message,
  tasks::{merge_left, merge_retry},
};

/// The run whose files are being put back, if any.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct PuttingBack(pub Option<RunId>);

impl DevtoolsInspectable for PuttingBack {}

/// The buttons' work in the project at `root`.
#[derive(Clone)]
pub(super) struct Actions {
  root: Arc<PathBuf>,
  programs: Programs,
  /// How a list of a failure's causes is joined.
  separator: Arc<str>,
  /// Why the last change asked for did not happen.
  pub failed: Signal<Option<Message>>,
  pub putting_back: Signal<PuttingBack>,
  /// Counts what makes the merges be read again.
  again: Signal<u64>,
}

impl Actions {
  pub(super) fn new(
    (root, programs): (&Path, Programs),
    separator: Arc<str>,
    (failed, putting_back, again): (Signal<Option<Message>>, Signal<PuttingBack>, Signal<u64>),
  ) -> Self {
    Self {
      root: Arc::new(root.to_path_buf()),
      programs,
      separator,
      failed,
      putting_back,
      again,
    }
  }

  /// *Try now*: Orchester's next try of `run`'s merge begins at once.
  pub(super) fn try_now(&self, run: RunId) {
    self.failed.set(None);
    if !merge_retry::try_now(run) {
      // The wait ended meanwhile: the try went on, or the run is Not merged as its last attempt left it.
      tracing::info!(%run, "Try now found no wait for the run's merge");
    }
    self.again.update(|count| *count += 1);
  }

  /// *Put files back* for `run`, on a thread of its own.
  pub(super) fn put_files_back(&self, run: RunId) {
    let this = self.clone();
    self.failed.set(None);
    self.putting_back.set(PuttingBack(Some(run)));
    let started = std::thread::Builder::new()
      .name("merges-put-back".to_owned())
      .spawn(move || {
        if let Err(problem) = merge_left::put_files_back(&this.root, &this.programs, run) {
          this.failed.set(Some(problem.message(&this.separator)));
        }
        this.putting_back.set(PuttingBack(None));
        this.again.update(|count| *count += 1);
      });
    if let Err(error) = started {
      tracing::error!(%error, "cannot start putting back the files a merge left changed");
      self.putting_back.set(PuttingBack(None));
      self.failed.set(Some(orchester_workspace::messages::interrupted()));
    }
  }
}
