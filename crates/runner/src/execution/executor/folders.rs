//! The desktop's requests about the folders the project's runs leave here ([`FoldersCommand`],
//! `docs/architecture/remote-runners.md`, rule 5): the sizes of warm workspaces and of the worktrees runs made before
//! workspaces, and cleaning, discarding or keeping a workspace's files with git. The desktop reserved the workspace in
//! the project database first and records what became of it; nothing here writes the database.

use std::{path::Path, time::Duration};

use orchester_core::{
  id::RunId,
  integration::{
    WORKTREES_DIR,
    workspace::{WorkspaceGitError, Workspaces},
  },
  run::BranchName,
  workspaces::WorkspaceNumber,
};
use orchester_exec::{ProcessRunner, disk_space::folder_size};
use orchester_ipc::{
  Rejection,
  folders::{FolderSizes, FoldersCommand, FoldersReply},
};

use crate::execution::{Scheduler, failed};

/// Deleting a large build folder can take a while; only a git that hangs is stopped.
const GIT_TIMEOUT: Duration = Duration::from_secs(60 * 60);
const GIT: &str = "git";

impl Scheduler {
  /// Carries out `command` on this project's folders, off the async workers: sizing walks folders and git blocks.
  pub async fn folders(&self, command: FoldersCommand) -> Result<FoldersReply, Rejection> {
    let root = self.inner.project_root.clone();
    tokio::task::spawn_blocking(move || serve(&root, command))
      .await
      .map_err(|error| failed("the folders request did not finish", &error))?
  }
}

fn serve(root: &Path, command: FoldersCommand) -> Result<FoldersReply, Rejection> {
  let runner = ProcessRunner::new(GIT_TIMEOUT);
  let workspaces = Workspaces::new(&runner, Path::new(GIT), root);
  let git_failed = |number: WorkspaceNumber| move |error: WorkspaceGitError| failed(&format!("workspace {number}"), &error);
  match command {
    FoldersCommand::Sizes { workspaces } => Ok(FoldersReply::Sizes(sizes(root, workspaces))),
    FoldersCommand::Clean { number } => {
      let path = number.path(root);
      let before = folder_size(&path).unwrap_or(0);
      workspaces.clean_warm(&path).map_err(git_failed(number))?;
      let freed = before.saturating_sub(folder_size(&path).unwrap_or(0));
      Ok(FoldersReply::Cleaned { freed })
    }
    FoldersCommand::Discard { number } => {
      workspaces.discard(&number.path(root)).map_err(git_failed(number))?;
      Ok(FoldersReply::Discarded)
    }
    FoldersCommand::Keep { number, branch } => {
      keep(&workspaces, root, number, &branch).map_err(git_failed(number))?;
      Ok(FoldersReply::Kept)
    }
  }
}

/// "Keep them in a branch" for the leftovers of workspace `number`.
fn keep(
  workspaces: &Workspaces<'_, ProcessRunner>,
  root: &Path,
  number: WorkspaceNumber,
  branch: &BranchName,
) -> Result<(), WorkspaceGitError> {
  workspaces.rescue(
    &number.path(root),
    branch,
    &format!("Kept from workspace {number}: what a stopped run left"),
  )
}

/// The sizes of `numbers`' folders and of every run's own worktree under `root`. A folder that cannot be read has
/// none; a worktrees folder that cannot be listed lists none, as there is then nothing to show.
fn sizes(root: &Path, numbers: Vec<WorkspaceNumber>) -> FolderSizes {
  let workspaces = numbers
    .into_iter()
    .map(|number| (number, folder_size(&number.path(root)).ok()))
    .collect();
  let worktrees = root.join(WORKTREES_DIR);
  let run_worktrees = std::fs::read_dir(&worktrees)
    .map(|entries| {
      entries
        .flatten()
        .filter_map(|entry| entry.file_name().to_str()?.parse::<RunId>().ok())
        .map(|run| (run, folder_size(&worktrees.join(run.to_string())).ok()))
        .collect()
    })
    .unwrap_or_default();
  FolderSizes {
    workspaces,
    run_worktrees,
  }
}
