//! Orchester's `delete_file` and `move_file` for agent CLIs: a deletion removes one entry of the worktree, a move
//! renames one to a path of the worktree where none is. Both change the entry they checked, a link itself and never
//! its target, through folders and handles that follow no link swapped in since ([`super::entry`]); the extra writable
//! folders are not theirs. A moved file is read again before it is edited.

use std::{
  fs,
  path::{Path, PathBuf},
};

use orchester_core::{
  connection::stream::{FileChange, FileChangeKind},
  native::{edit::LineChanges, session::ToolOutcome},
};

use super::{
  FileError, FileTools, done,
  entry::{self, EntryError},
  outcome,
  write::changed_lines,
};

impl FileTools {
  /// Orchester's `delete_file`: the entry `filename` names goes.
  pub fn delete_file(&self, filename: &str) -> ToolOutcome {
    outcome(self.delete(filename))
  }

  /// Orchester's `move_file`: the entry at `from` moves to `to`, where none may be.
  pub fn move_file(&self, from: &str, to: &str) -> ToolOutcome {
    outcome(self.rename(from, to))
  }

  fn delete(&self, filename: &str) -> Result<ToolOutcome, FileError> {
    let path = self.entry(filename)?;
    let shown = self.shown(&path);
    entry::remove(self.scope.worktree(), &path).map_err(|error| self.entry_error(&path, "deleted", error))?;
    let summary = format!("deleted {shown}");
    let change = FileChange {
      path: shown,
      kind: FileChangeKind::Deleted,
    };
    let detail = changed_lines(FileChangeKind::Deleted, LineChanges::default());
    Ok(done(summary.clone(), summary, vec![change], detail))
  }

  fn rename(&self, from: &str, to: &str) -> Result<ToolOutcome, FileError> {
    let source = self.entry(from)?;
    let target = self.scope.writable(to)?;
    if !target.starts_with(self.scope.worktree()) {
      return Err(FileError::NotInWorktree(self.shown(&target)));
    }
    if let Some(parent) = target.parent() {
      fs::create_dir_all(parent).map_err(|error| self.io_error(parent, "created", &error))?;
    }
    let target = self.entry(to)?;
    let (shown_from, shown_to) = (self.shown(&source), self.shown(&target));
    entry::rename(self.scope.worktree(), &source, &target).map_err(|error| match error {
      EntryError::Exists => FileError::Exists(shown_to.clone()),
      other => self.entry_error(&source, "moved", other),
    })?;
    let summary = format!("moved {shown_from} to {shown_to}");
    let changes = vec![
      FileChange {
        path: shown_from,
        kind: FileChangeKind::Deleted,
      },
      FileChange {
        path: shown_to,
        kind: FileChangeKind::Added,
      },
    ];
    let detail = changed_lines(FileChangeKind::Added, LineChanges::default());
    Ok(done(summary.clone(), summary, changes, detail))
  }

  /// The entry `requested` names in the worktree: its folder resolved, its last name not followed, so that a link is
  /// the entry itself. The entry, followed, must be one the step may change; the folder must be the worktree's.
  fn entry(&self, requested: &str) -> Result<PathBuf, FileError> {
    let target = self.scope.writable(requested)?;
    let path = Path::new(requested);
    let (Some(name), Some(parent)) = (path.file_name(), path.parent()) else {
      return Err(FileError::NotRegular(self.shown(&target)));
    };
    let parent = if parent.as_os_str().is_empty() {
      "."
    } else {
      parent
        .to_str()
        .ok_or_else(|| FileError::NotRegular(self.shown(&target)))?
    };
    let folder = self.scope.writable(parent)?;
    if !folder.starts_with(self.scope.worktree()) {
      return Err(FileError::NotInWorktree(self.shown(&target)));
    }
    Ok(folder.join(name))
  }

  fn entry_error(&self, path: &Path, action: &'static str, error: EntryError) -> FileError {
    let shown = self.shown(path);
    match error {
      EntryError::Moved => FileError::LinkOnTheWay(shown),
      EntryError::Folder => FileError::Folder(shown),
      EntryError::Exists => FileError::Exists(shown),
      EntryError::Io(error) => self.io_error(path, action, &error),
    }
  }
}
