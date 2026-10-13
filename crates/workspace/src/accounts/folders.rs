//! The file system side of account folders: the CLI's own folder, and whether a brought folder lies in a repository
//! or a project. The rules themselves are `orchester_core::account`'s.

use std::{
  env,
  path::{Path, PathBuf},
};

use orchester_core::account::{AccountCli, AgentAccount, FolderProblem, brought_folder_problem};
use orchester_store::RegisteredProject;

/// The home folder, as the CLIs find it.
fn home() -> Option<PathBuf> {
  env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

/// The folder Default uses: the CLI's variable as Orchester's environment has it, else `~/.claude` / `~/.codex`.
pub fn own_folder(cli: AccountCli) -> Option<PathBuf> {
  env::var_os(cli.folder_variable())
    .filter(|value| !value.is_empty())
    .map(PathBuf::from)
    .or_else(|| Some(home()?.join(cli.own_folder_name())))
}

/// What is wrong with `folder` as a brought folder of an account of `cli` beside `accounts`, if anything.
pub fn brought_problem(
  cli: AccountCli,
  folder: &Path,
  accounts: &[AgentAccount],
  projects: &[RegisteredProject],
) -> Option<FolderProblem> {
  let repository = repository_of(folder, projects);
  let own = own_folder(cli);
  brought_folder_problem(
    folder,
    repository.as_deref(),
    accounts,
    own.as_deref(),
    home().as_deref(),
  )
}

/// The project or git repository `folder` lies in: a listed project's folder, or the nearest folder above it (or
/// itself) with a `.git` entry, which a worktree has as a file.
fn repository_of(folder: &Path, projects: &[RegisteredProject]) -> Option<PathBuf> {
  if let Some(project) = projects.iter().find(|project| folder.starts_with(&project.repo_root)) {
    return Some(project.repo_root.clone());
  }
  folder
    .ancestors()
    .find(|ancestor| ancestor.join(".git").exists())
    .map(Path::to_path_buf)
}
