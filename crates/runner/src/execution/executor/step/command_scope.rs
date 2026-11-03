//! Where the commands of a step's agent run and may write, as the runner reads them to judge its requests.

use std::path::{Path, PathBuf};

use orchester_core::{
  permission::{LaunchPermissions, category::CommandScope},
  resource::CommandEnvironment,
};

/// Where the commands of an agent working in `worktree` with `launch` permissions run and may write: the worktree
/// and the extra writable folders it was given, as written (relative to the repository `root`), for the host's user
/// whose agent gets the project's `environment` too.
pub(super) fn command_scope(
  root: &Path,
  worktree: &Path,
  launch: &LaunchPermissions,
  environment: &CommandEnvironment,
) -> CommandScope {
  let writable: Vec<PathBuf> = launch
    .permissions()
    .granted_folders()
    .map(|folders| {
      folders
        .entries()
        .iter()
        .map(|folder| folder.lexical_path(root))
        .collect()
    })
    .unwrap_or_default();
  CommandScope::on_host(worktree, &writable).with_project_environment(environment)
}
