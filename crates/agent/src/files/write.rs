//! `edit_file` and `write_file` (`docs/architecture/native-agent.md` §4.4, §4.5, §5.1). A file is changed
//! only when its read stamp still matches it, and always atomically: the new content goes to a temporary
//! file in the same folder, is flushed, and is renamed over the target, which replaces the directory
//! entry instead of writing through a link. The target is resolved again just before the rename, since
//! a command may have changed the folders in between.

use std::{
  fs,
  io::{self, Write as _},
  path::Path,
};

use orchester_core::{
  connection::stream::{FileChange, FileChangeKind},
  native::{
    edit::apply_edits,
    session::ToolOutcome,
    tools::{EditFile, WriteFile},
  },
};

use super::{FileError, FileTools, done, outcome, stamp::ReadStamp};

impl FileTools {
  pub fn edit_file(&self, input: &EditFile) -> ToolOutcome {
    outcome(self.edit(input))
  }

  pub fn write_file(&self, input: &WriteFile) -> ToolOutcome {
    outcome(self.write(input))
  }

  fn edit(&self, input: &EditFile) -> Result<ToolOutcome, FileError> {
    let path = self.scope.writable(input.path())?;
    let shown = self.shown(&path);
    let metadata = self.existing(&path)?.ok_or_else(|| FileError::Missing(shown.clone()))?;
    let bytes = self.checked_content(&path, &metadata)?;
    let text = String::from_utf8(bytes).map_err(|_| FileError::Io {
      path: shown.clone(),
      action: "edited as text",
      kind: io::ErrorKind::InvalidData,
    })?;
    let changed = apply_edits(&text, input.edits())?;
    self.replace(&path, input.path(), changed.as_bytes(), Some(&metadata))?;
    let count = input.edits().len();
    let summary = format!("edited {shown}: {count} edit(s) applied");
    let change = FileChange {
      path: shown,
      kind: FileChangeKind::Modified,
    };
    Ok(done(summary.clone(), summary, vec![change]))
  }

  fn write(&self, input: &WriteFile) -> Result<ToolOutcome, FileError> {
    let path = self.scope.writable(input.path())?;
    let shown = self.shown(&path);
    let existing = self.existing(&path)?;
    let kind = match &existing {
      Some(metadata) => {
        self.checked_content(&path, metadata)?;
        FileChangeKind::Modified
      }
      None => {
        if let Some(parent) = path.parent() {
          fs::create_dir_all(parent).map_err(|error| self.io_error(parent, "created", &error))?;
        }
        FileChangeKind::Added
      }
    };
    self.replace(&path, input.path(), input.content().as_bytes(), existing.as_ref())?;
    let summary = format!("wrote {shown}: {} bytes", input.content().len());
    let change = FileChange { path: shown, kind };
    Ok(done(summary.clone(), summary, vec![change]))
  }

  /// The metadata of the file at `path`; `None` when there is none.
  fn existing(&self, path: &Path) -> Result<Option<fs::Metadata>, FileError> {
    match fs::metadata(path) {
      Ok(metadata) if metadata.is_dir() => Err(FileError::Folder(self.shown(path))),
      Ok(metadata) if !metadata.is_file() => Err(FileError::NotRegular(self.shown(path))),
      Ok(metadata) => Ok(Some(metadata)),
      Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
      Err(error) => Err(self.io_error(path, "read", &error)),
    }
  }

  /// The content of the file at `path`, when the agent's read stamp still matches it.
  fn checked_content(&self, path: &Path, metadata: &fs::Metadata) -> Result<Vec<u8>, FileError> {
    let shown = self.shown(path);
    let stamp = self.stamps.get(path).ok_or_else(|| FileError::NotRead(shown.clone()))?;
    let bytes = self.content(path, metadata)?;
    if ReadStamp::of(&bytes, metadata) != stamp {
      return Err(FileError::Stale(shown));
    }
    Ok(bytes)
  }

  /// Puts `bytes` at `path` atomically and stamps the result; `requested` is the path as the call gave
  /// it, resolved again here.
  fn replace(
    &self,
    path: &Path,
    requested: &str,
    bytes: &[u8],
    existing: Option<&fs::Metadata>,
  ) -> Result<(), FileError> {
    if self.scope.writable(requested)? != path {
      return Err(FileError::Moved(self.shown(path)));
    }
    let folder = path.parent().ok_or_else(|| FileError::NotFolder(self.shown(path)))?;
    let fail = |error: io::Error| self.io_error(path, "written", &error);
    let mut temporary = tempfile::Builder::new()
      .prefix(".orchester-write-")
      .tempfile_in(folder)
      .map_err(fail)?;
    temporary.write_all(bytes).map_err(fail)?;
    temporary.as_file().sync_all().map_err(fail)?;
    match existing {
      Some(metadata) => fs::set_permissions(temporary.path(), metadata.permissions()).map_err(fail)?,
      None => permissions::new_file(temporary.path()).map_err(fail)?,
    }
    // A new file must not replace one that appeared since it was looked for: the agent never read it.
    let persisted = if existing.is_some() {
      temporary.persist(path)
    } else {
      temporary.persist_noclobber(path)
    };
    persisted.map_err(|error| match error.error.kind() {
      io::ErrorKind::AlreadyExists => FileError::Appeared(self.shown(path)),
      _ => fail(error.error),
    })?;
    let metadata = fs::metadata(path).map_err(fail)?;
    self.stamps.record(path, ReadStamp::of(bytes, &metadata));
    Ok(())
  }
}

/// The permissions of a new file.
#[cfg(unix)]
mod permissions {
  use std::{fs, io, os::unix::fs::PermissionsExt, path::Path};

  /// A temporary file is private to its owner (0600); a new file gets the usual 0644.
  pub(super) fn new_file(path: &Path) -> io::Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o644))
  }
}

/// The permissions of a new file.
#[cfg(windows)]
mod permissions {
  use std::{io, path::Path};

  /// The temporary file got the access rules its folder hands down, as a file created there directly
  /// would; nothing to change.
  pub(super) const fn new_file(_path: &Path) -> io::Result<()> {
    Ok(())
  }
}
