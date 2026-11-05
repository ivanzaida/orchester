//! The desktop's requests about the folders the project's runs leave here ([`FoldersCommand`],
//! `docs/architecture/remote-runners.md`, rules 5 and 6): the sizes of warm workspaces and of the worktrees runs made
//! before workspaces, and cleaning, discarding or keeping a workspace's files with git, each folder taken from the
//! execution root. The desktop reserved the workspace in the project database first and records what became of it;
//! nothing here writes the database.

use std::{path::Path, time::Duration};

use orchester_core::{
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

use crate::{
  execution::{Scheduler, failed},
  execution_root::ExecutionRoot,
};

/// Deleting a large build folder can take a while; only a git that hangs is stopped.
const GIT_TIMEOUT: Duration = Duration::from_secs(60 * 60);
const GIT: &str = "git";

impl Scheduler {
  /// Carries out `command` on this project's folders, off the async workers: sizing walks folders and git blocks.
  pub async fn folders(&self, command: FoldersCommand) -> Result<FoldersReply, Rejection> {
    let execution = ExecutionRoot::local(&self.inner.project_root);
    tokio::task::spawn_blocking(move || serve(&execution, command))
      .await
      .map_err(|error| failed("the folders request did not finish", &error))?
  }
}

fn serve(execution: &ExecutionRoot, command: FoldersCommand) -> Result<FoldersReply, Rejection> {
  let runner = ProcessRunner::new(GIT_TIMEOUT);
  let workspaces = Workspaces::new(&runner, Path::new(GIT), execution.repository());
  let git_failed =
    |number: WorkspaceNumber| move |error: WorkspaceGitError| failed(&format!("workspace {number}"), &error);
  match command {
    FoldersCommand::Sizes { workspaces } => Ok(FoldersReply::Sizes(sizes(execution, workspaces))),
    FoldersCommand::Clean { number } => {
      let path = execution.workspace(number);
      let before = folder_size(&path).unwrap_or(0);
      workspaces.clean_warm(&path).map_err(git_failed(number))?;
      let freed = before.saturating_sub(folder_size(&path).unwrap_or(0));
      Ok(FoldersReply::Cleaned { freed })
    }
    FoldersCommand::Discard { number } => {
      workspaces
        .discard(&execution.workspace(number))
        .map_err(git_failed(number))?;
      Ok(FoldersReply::Discarded)
    }
    FoldersCommand::Keep { number, branch } => {
      keep(&workspaces, execution, number, &branch).map_err(git_failed(number))?;
      Ok(FoldersReply::Kept)
    }
  }
}

/// "Keep them in a branch" for the leftovers of workspace `number`.
fn keep(
  workspaces: &Workspaces<'_, ProcessRunner>,
  execution: &ExecutionRoot,
  number: WorkspaceNumber,
  branch: &BranchName,
) -> Result<(), WorkspaceGitError> {
  workspaces.rescue(
    &execution.workspace(number),
    branch,
    &format!("Kept from workspace {number}: what a stopped run left"),
  )
}

/// The sizes of `numbers`' folders and of every folder of the worktrees folder, runs' own worktrees and their steps'.
/// A folder that cannot be read has none; a worktrees folder that cannot be listed lists none, as there is then
/// nothing to show.
fn sizes(execution: &ExecutionRoot, numbers: Vec<WorkspaceNumber>) -> FolderSizes {
  let workspaces = numbers
    .into_iter()
    .map(|number| (number, folder_size(&execution.workspace(number)).ok()))
    .collect();
  let worktrees = std::fs::read_dir(execution.repository().join(WORKTREES_DIR))
    .map(|entries| {
      entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| {
          let name = entry.file_name().into_string().ok()?;
          Some((name, folder_size(&entry.path()).ok()))
        })
        .collect()
    })
    .unwrap_or_default();
  FolderSizes { workspaces, worktrees }
}
