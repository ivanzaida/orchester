//! Orchester's `edit_file`, `create_file`, `delete_file` and `move_file` for agent CLIs: an edit replaces a range of
//! lines only when the file was read in this invocation, has not changed since that read, and the range holds exactly
//! the lines the call expects (`orchester_core::orchester_tools::replace_lines`); otherwise nothing is written and the
//! answer says what the range holds now. A new file is made only where none exists. Both write atomically, as the
//! native agent's tools do ([`FileTools::replace`]). A deletion removes one file; a move renames one file to a path
//! where none is (a file that appears there between the check and the rename is replaced), and the moved file is read
//! again before it is edited.

use std::{fs, io};

use orchester_core::{
  connection::stream::{FileChange, FileChangeKind},
  native::{
    edit::{Edited, LineChanges},
    session::ToolOutcome,
  },
  orchester_tools::{LineEdit, NewFile, range_now, replace_lines},
};

use super::{FileError, FileTools, count, done, outcome, stamp::ReadStamp, write::changed_lines};

impl FileTools {
  /// Orchester's `edit_file`: lines `start_line..=end_line` become the call's content.
  pub fn edit_lines(&self, input: &LineEdit) -> ToolOutcome {
    outcome(self.edit_range(input))
  }

  /// Orchester's `create_file`: a new file, refused where one exists.
  pub fn create_file(&self, input: &NewFile) -> ToolOutcome {
    outcome(self.create(input))
  }

  /// Orchester's `delete_file`: the file at `filename` goes.
  pub fn delete_file(&self, filename: &str) -> ToolOutcome {
    outcome(self.delete(filename))
  }

  /// Orchester's `move_file`: the file at `from` moves to `to`, where no file may be.
  pub fn move_file(&self, from: &str, to: &str) -> ToolOutcome {
    outcome(self.rename(from, to))
  }

  fn delete(&self, filename: &str) -> Result<ToolOutcome, FileError> {
    let path = self.scope.writable(filename)?;
    let shown = self.shown(&path);
    self.existing(&path)?.ok_or_else(|| FileError::Missing(shown.clone()))?;
    fs::remove_file(&path).map_err(|error| self.io_error(&path, "deleted", &error))?;
    let summary = format!("deleted {shown}");
    let change = FileChange {
      path: shown,
      kind: FileChangeKind::Deleted,
    };
    Ok(done(
      summary.clone(),
      summary,
      vec![change],
      changed_lines(FileChangeKind::Deleted, LineChanges::default()),
    ))
  }

  fn rename(&self, from: &str, to: &str) -> Result<ToolOutcome, FileError> {
    let (source, target) = (self.scope.writable(from)?, self.scope.writable(to)?);
    let (shown_from, shown_to) = (self.shown(&source), self.shown(&target));
    self
      .existing(&source)?
      .ok_or_else(|| FileError::Missing(shown_from.clone()))?;
    if self.existing(&target)?.is_some() {
      return Err(FileError::Exists(shown_to));
    }
    if let Some(parent) = target.parent() {
      fs::create_dir_all(parent).map_err(|error| self.io_error(parent, "created", &error))?;
    }
    fs::rename(&source, &target).map_err(|error| self.io_error(&source, "moved", &error))?;
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

  fn edit_range(&self, input: &LineEdit) -> Result<ToolOutcome, FileError> {
    let path = self.scope.writable(input.filename())?;
    let shown = self.shown(&path);
    let metadata = self
      .existing(&path)?
      .ok_or_else(|| FileError::NoFileToEdit(shown.clone()))?;
    let stamp = self
      .stamps
      .get(&path)
      .ok_or_else(|| FileError::NotRead(shown.clone()))?;
    let bytes = self.content(&path, &metadata)?;
    let text = String::from_utf8(bytes).map_err(|_| FileError::Io {
      path: shown.clone(),
      action: "edited as text",
      kind: io::ErrorKind::InvalidData,
    })?;
    if ReadStamp::of(text.as_bytes(), &metadata) != stamp {
      return Err(FileError::ChangedSinceRead {
        path: shown,
        now: range_now(&text, input.start_line(), input.end_line()),
      });
    }
    let Edited { text: changed, lines } = replace_lines(&text, input)?;
    self.replace(&path, input.filename(), changed.as_bytes(), Some(&metadata))?;
    let total = changed.lines().count();
    let added = input.content().lines().count();
    let summary = if added == 0 {
      format!(
        "edited {shown}: lines {}–{} removed; the file has {total} lines now",
        input.start_line(),
        input.end_line()
      )
    } else {
      format!(
        "edited {shown}: lines {}–{} now hold the {added} new lines; the file has {total} lines now",
        input.start_line(),
        u64::from(input.start_line()) + count(added) - 1
      )
    };
    let change = FileChange {
      path: shown,
      kind: FileChangeKind::Modified,
    };
    Ok(done(
      summary.clone(),
      summary,
      vec![change],
      changed_lines(FileChangeKind::Modified, lines),
    ))
  }

  fn create(&self, input: &NewFile) -> Result<ToolOutcome, FileError> {
    let path = self.scope.writable(input.filename())?;
    let shown = self.shown(&path);
    if self.existing(&path)?.is_some() {
      return Err(FileError::Exists(shown));
    }
    if let Some(parent) = path.parent() {
      fs::create_dir_all(parent).map_err(|error| self.io_error(parent, "created", &error))?;
    }
    self.replace(&path, input.filename(), input.content().as_bytes(), None)?;
    let summary = format!("created {shown}: {} lines", input.content().lines().count());
    let change = FileChange {
      path: shown,
      kind: FileChangeKind::Added,
    };
    let lines = LineChanges::between("", input.content());
    Ok(done(
      summary.clone(),
      summary,
      vec![change],
      changed_lines(FileChangeKind::Added, lines),
    ))
  }
}
