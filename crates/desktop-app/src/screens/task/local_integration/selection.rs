use std::ops::Deref;

use lurq::app::component::DevtoolsInspectable;
use orchester_core::integration::local::LocalPath;
use orchester_workspace::tasks::local_integration::{ShownSnapshot, Snapshot};

/// The reviewed snapshots the person selected, each on its own.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Selection(Vec<ShownSnapshot>);
impl DevtoolsInspectable for Selection {}
impl Deref for Selection {
  type Target = [ShownSnapshot];
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl Selection {
  pub(super) fn has(&self, path: &LocalPath, snapshot: Snapshot) -> bool {
    self
      .0
      .iter()
      .any(|chosen| chosen.path == *path && chosen.snapshot == snapshot)
  }

  pub(super) fn choose(&mut self, path: &LocalPath, snapshot: Snapshot, on: bool) {
    self
      .0
      .retain(|chosen| chosen.path != *path || chosen.snapshot != snapshot);
    if on {
      self.0.push(ShownSnapshot {
        path: path.clone(),
        snapshot,
      });
    }
  }

  /// The paths a commit takes: those whose index or new-file snapshot is selected. A working-copy snapshot is
  /// never committed.
  pub(super) fn commit_paths(&self) -> Vec<LocalPath> {
    self.distinct(|snapshot| snapshot != Snapshot::Unstaged)
  }

  /// The paths a preservation keeps, whole: those with any selected snapshot.
  pub(super) fn paths(&self) -> Vec<LocalPath> {
    self.distinct(|_| true)
  }

  fn distinct(&self, taken: impl Fn(Snapshot) -> bool) -> Vec<LocalPath> {
    let mut paths: Vec<LocalPath> = Vec::new();
    for chosen in self.0.iter().filter(|chosen| taken(chosen.snapshot)) {
      if !paths.contains(&chosen.path) {
        paths.push(chosen.path.clone());
      }
    }
    paths
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ShownFile(pub Option<ShownSnapshot>);
impl DevtoolsInspectable for ShownFile {}
