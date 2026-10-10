//! The files the Changes tab lists: the run's checkpointed changes, then what its worktree holds beyond
//! them under a group of their own (`CHECKPOINTED` and `NOT CHECKPOINTED` in `qT0kd`). A file can be in
//! both. The path names a file by its repository path and where its change is read from
//! (`routes::ChangedFile`), so going back returns to it.

use std::sync::Arc;

use orchester_workspace::tasks::changes::ReadChanges;

use crate::routes::{ChangeSource, ChangedFile};

/// A file of the list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed {
  pub source: ChangeSource,
  /// Position of the file in its diff.
  pub index: usize,
  pub path: Arc<str>,
}

/// The diffs the list was made of, by their keys: the same keys, the same list.
pub type ListKeys = (Option<Arc<str>>, Option<Arc<str>>);

/// The files of the checkpointed changes and of the worktree's, in that order.
#[derive(Clone, Debug)]
pub struct FileListing {
  pub keys: ListKeys,
  pub files: Arc<Vec<Listed>>,
}

impl FileListing {
  pub fn of(checkpointed: Option<&ReadChanges>, worktree: Option<&ReadChanges>) -> Self {
    let parts = [
      (checkpointed, ChangeSource::Checkpoint),
      (worktree, ChangeSource::Worktree),
    ];
    let files = parts
      .into_iter()
      .filter_map(|(changes, source)| Some((changes?, source)))
      .flat_map(|(changes, source)| {
        changes
          .diff
          .files()
          .iter()
          .enumerate()
          .map(move |(index, file)| Listed {
            source,
            index,
            path: file.path().into(),
          })
      })
      .collect();
    Self {
      keys: (
        checkpointed.map(|changes| changes.key.clone()),
        worktree.map(|changes| changes.key.clone()),
      ),
      files: Arc::new(files),
    }
  }

  pub fn is_empty(&self) -> bool {
    self.files.is_empty()
  }

  /// How many files come from `source`.
  pub fn count(&self, source: ChangeSource) -> usize {
    self.files.iter().filter(|file| file.source == source).count()
  }

  /// The list position of `file`: its change from where it names, else its other one (a checkpointed file
  /// the worktree reverted, a worktree file a new checkpoint took), else the first file.
  pub fn position(&self, file: Option<&ChangedFile>) -> usize {
    let Some(file) = file else {
      return 0;
    };
    let find = |source: ChangeSource| {
      self
        .files
        .iter()
        .position(|listed| listed.source == source && *listed.path == *file.path())
    };
    let other = match file.source() {
      ChangeSource::Checkpoint => ChangeSource::Worktree,
      ChangeSource::Worktree => ChangeSource::Checkpoint,
    };
    find(file.source()).or_else(|| find(other)).unwrap_or_default()
  }
}
