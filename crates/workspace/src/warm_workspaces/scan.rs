//! The git-ignored folders of the person's own checkout, read only to suggest warm folders (ORC-114): git lists them
//! up to [`SUGGEST_DEPTH`] deep, and each is sized. Nothing in the checkout is written.

use std::{path::Path, time::Duration};

use orchester_core::{
  command::{Command, CommandRunner},
  workspaces::{IgnoredFolder, SUGGEST_DEPTH},
};
use orchester_exec::{ProcessRunner, disk_space::folder_size};

use crate::tasks::changes::GIT;

/// Listing a large checkout's ignored files can take a while; a git that hangs suggests nothing.
const LIST_TIMEOUT: Duration = Duration::from_secs(60);

/// The ignored folders of the checkout at `repo_root` up to [`SUGGEST_DEPTH`] deep, with their sizes; none when git
/// cannot list them, which is logged. `.orchester/` is Orchester's own and never suggested.
pub fn ignored_folders(repo_root: &Path) -> Vec<IgnoredFolder> {
  let args = [
    "ls-files",
    "--others",
    "--ignored",
    "--exclude-standard",
    "--directory",
    "--no-empty-directory",
    "-z",
  ];
  let command = Command::new(Path::new(GIT), args).with_current_dir(repo_root);
  let listed = match ProcessRunner::new(LIST_TIMEOUT).run(&command) {
    Ok(output) if output.success() => output.stdout,
    Ok(output) => {
      tracing::warn!(stderr = %output.stderr, "git cannot list the checkout's ignored folders");
      return Vec::new();
    }
    Err(error) => {
      tracing::warn!(%error, "cannot run git to list the checkout's ignored folders");
      return Vec::new();
    }
  };
  listed
    .split('\0')
    .filter(|entry| entry.ends_with('/') && !entry.starts_with(".orchester/"))
    .filter(|entry| entry.trim_end_matches('/').split('/').count() <= SUGGEST_DEPTH)
    .map(|entry| IgnoredFolder {
      path: entry.to_owned(),
      bytes: folder_size(&repo_root.join(entry.trim_end_matches('/'))).unwrap_or(0),
    })
    .collect()
}
