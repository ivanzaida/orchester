//! Files that hold secret values for one agent invocation: the MCP configuration of a Claude Code
//! invocation (`orchester_core::connection::invocation::ClaudeMcpConfig`). Each lives in a folder of
//! the project's own under the user's temporary folder, is readable by the user only (see the
//! platform modules), and is removed when its [`PrivateFile`] is dropped, which is when the CLI that
//! read it has ended. A runner that ended abnormally leaves its files behind; the next runner of the
//! project removes them when it starts ([`remove_leftovers`]).

#[cfg(not(windows))]
mod unix;
#[cfg(windows)]
mod windows;

use std::{
  fs,
  io::{self, Write},
  path::{Path, PathBuf},
};

use orchester_core::id::ProjectId;
#[cfg(not(windows))]
use unix as platform;
#[cfg(windows)]
use windows as platform;

/// The folder of `project`'s private files.
pub fn directory(project: ProjectId) -> PathBuf {
  std::env::temp_dir().join("orchester-mcp").join(project.to_string())
}

/// Removes the files a runner of `project` left behind.
pub fn remove_leftovers(project: ProjectId) -> io::Result<()> {
  match fs::remove_dir_all(directory(project)) {
    Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
    _ => Ok(()),
  }
}

/// A file with secret contents, removed when this is dropped.
#[derive(Debug)]
pub struct PrivateFile {
  path: PathBuf,
}

impl PrivateFile {
  /// Writes `contents` to a new file `name` in `project`'s folder; a file of that name must not
  /// exist.
  pub fn write(project: ProjectId, name: &str, contents: &str) -> io::Result<Self> {
    let folder = directory(project);
    platform::create_folder(&folder)?;
    let path = folder.join(name);
    let mut handle = platform::create_file(&path)?;
    let file = Self { path };
    let written = handle.write_all(contents.as_bytes()).and_then(|()| handle.sync_all());
    // Closed first: Windows does not remove an open file, and `file` removes it when writing failed.
    drop(handle);
    written?;
    Ok(file)
  }

  pub fn path(&self) -> &Path {
    &self.path
  }
}

impl Drop for PrivateFile {
  fn drop(&mut self) {
    if let Err(error) = fs::remove_file(&self.path)
      && error.kind() != io::ErrorKind::NotFound
    {
      tracing::warn!(path = %self.path.display(), %error, "cannot remove a file with secret values");
    }
  }
}
