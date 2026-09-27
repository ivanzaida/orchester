//! The files the Changes tab lists: the run's checkpointed changes, then the changes in its
//! worktree that are not checkpointed, under a heading of their own. A file is found by its path,
//! as an Activity entry or the tab's path names it.

use std::sync::Arc;

use lurq::app::ctx::Ctx;

use crate::{
  navigation::route::ChangedFile,
  tasks::changes::ReadChanges,
  ui::diff::{FileEntry, FileGroup},
};

/// Element id prefix of the files not checkpointed (`diff-file` is the checkpointed ones').
const WORKTREE_FILES: &str = "diff-worktree-file";
const CHECKPOINTED_FILES: &str = "diff-file";

/// Where a file of the list comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Source {
  pub(super) worktree: bool,
  /// Position of the file in its diff.
  pub(super) index: usize,
  pub(super) path: Arc<str>,
}

/// The groups of the file list, and where each listed file comes from, by list position.
#[derive(Clone, Debug)]
pub(super) struct Listed {
  /// The diffs the list was made of, by their keys.
  pub(super) keys: (Option<Arc<str>>, Option<Arc<str>>),
  pub(super) groups: Arc<Vec<FileGroup>>,
  pub(super) sources: Arc<Vec<Source>>,
}

impl Listed {
  /// The list of `checkpointed` and `worktree` changes, each group only when it has files.
  pub(super) fn of(ctx: &Ctx, checkpointed: Option<&ReadChanges>, worktree: Option<&ReadChanges>) -> Self {
    let mut groups = Vec::new();
    let mut sources = Vec::new();
    let parts = [
      (checkpointed, false, CHECKPOINTED_FILES, "diff.files.heading"),
      (worktree, true, WORKTREE_FILES, "runs.changes.notCheckpointedHeading"),
    ];
    for (changes, from_worktree, key, heading) in parts {
      let Some(changes) = changes.filter(|changes| !changes.diff.files().is_empty()) else {
        continue;
      };
      let count = changes.diff.files().len().to_string();
      let mut files = Vec::new();
      for (index, file) in changes.diff.files().iter().enumerate() {
        let entry = FileEntry::of(sources.len(), file);
        sources.push(Source {
          worktree: from_worktree,
          index,
          path: entry.path.clone(),
        });
        files.push(entry);
      }
      groups.push(FileGroup {
        key,
        heading: ctx.t_args(heading, [("count", count.as_str())]),
        files,
      });
    }
    Self {
      keys: (
        checkpointed.map(|changes| changes.key.clone()),
        worktree.map(|changes| changes.key.clone()),
      ),
      groups: Arc::new(groups),
      sources: Arc::new(sources),
    }
  }

  pub(super) fn is_empty(&self) -> bool {
    self.sources.is_empty()
  }

  /// The list position of `file`: a file not checkpointed only among those; otherwise its
  /// checkpointed change first. The first file when the list does not hold it.
  pub(super) fn position(&self, file: Option<&ChangedFile>) -> usize {
    let Some(file) = file else {
      return 0;
    };
    let find = |worktree: bool| {
      self
        .sources
        .iter()
        .position(|source| source.worktree == worktree && *source.path == *file.path)
    };
    let found = if file.worktree {
      find(true)
    } else {
      find(false).or_else(|| find(true))
    };
    found.unwrap_or_default()
  }
}
