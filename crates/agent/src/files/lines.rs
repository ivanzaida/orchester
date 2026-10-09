//! Orchester's `edit_file` and `create_file` for agent CLIs: an edit replaces a range of lines only when the file was
//! read in this invocation, has not changed since that read, and the range holds exactly the lines the call expects
//! (`orchester_core::orchester_tools::replace_lines`); otherwise nothing is written and the answer says what the range
//! holds now. A new file is made only where none exists. Both write atomically, as the native agent's tools do
//! ([`FileTools::replace`]).

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
