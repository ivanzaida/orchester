//! What a run's folder holds beyond its last commit, read with git where the folder is, for the desktop's start-up
//! recovery ([`orchester_ipc::Command::RunFolderState`]): the desktop reaches a run's worktree only through the runner
//! (`docs/architecture/remote-runners.md`, rule 5).

use std::{
  path::{Path, PathBuf},
  time::Duration,
};

use orchester_core::{command::CommandRunner, diff::git::git_command};
use orchester_exec::ProcessRunner;
use orchester_ipc::FolderState;

/// `git status` only reads; one that takes this long hangs.
const STATUS_TIMEOUT: Duration = Duration::from_secs(120);
const RUNNER: ProcessRunner = ProcessRunner::new(STATUS_TIMEOUT);
const GIT: &str = "git";

/// The state of the first of `folders`, in the order the run's folder is looked for, that is a folder; unknown when
/// none is, or git cannot tell. Blocks on git.
pub(in crate::execution) fn read(folders: &[PathBuf]) -> FolderState {
  let Some(folder) = folders.iter().find(|folder| folder.is_dir()) else {
    return FolderState::Unknown;
  };
  let status = git_command(
    Path::new(GIT),
    folder,
    ["status", "--porcelain", "--untracked-files=all"],
  );
  match RUNNER.run(&status) {
    Ok(output) if output.success() => from_porcelain(&output.stdout),
    Ok(output) => {
      tracing::warn!(folder = %folder.display(), stderr = %output.stderr, "git status failed");
      FolderState::Unknown
    }
    Err(error) => {
      tracing::warn!(folder = %folder.display(), %error, "cannot run git status");
      FolderState::Unknown
    }
  }
}

/// The state `git status --porcelain --untracked-files=all` printed as `stdout`: the files changed or untracked, and
/// the first of them as a repository path.
fn from_porcelain(stdout: &str) -> FolderState {
  let files: Vec<&str> = stdout.lines().filter(|line| !line.trim().is_empty()).collect();
  // `XY path`, or `XY from -> to` for a rename: the path the folder has now.
  let first = files.first().and_then(|line| line.get(3..)).map(|path| {
    let path = path.rsplit(" -> ").next().unwrap_or(path);
    path.trim_matches('"').to_owned()
  });
  match files.len() {
    0 => FolderState::Clean,
    files => FolderState::Uncommitted { files, first },
  }
}
