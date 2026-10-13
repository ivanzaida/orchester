//! The file system side of account folders: the CLI's own folder, whether a brought folder lies in a repository or
//! a project, and whether a folder Orchester created is still a real folder where it was made. Folders are compared
//! as the file system resolves them (links, junctions and case), so two spellings of one folder are one. The rules
//! themselves are `orchester_core::account`'s.

use std::{
  env, fs,
  path::{Path, PathBuf},
};

use orchester_core::account::{
  AccountCli, AgentAccount, FolderProblem, brought_folder_problem, plain_full_path, resolved, same_folder, within,
};
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

/// What is wrong with `folder` as a brought folder of an account beside `accounts` (every CLI's) and each CLI's own
/// folder, if anything; every path is compared as the file system resolves it.
pub fn brought_problem(
  folder: &Path,
  accounts: &[AgentAccount],
  projects: &[RegisteredProject],
) -> Option<FolderProblem> {
  let repository = plain_full_path(folder)
    .then(|| repository_of(&found(folder), projects))
    .flatten();
  brought_folder_problem(
    folder,
    repository.as_deref(),
    (accounts, &own_folder),
    home().as_deref(),
    &found,
  )
}

/// `path` as the file system resolves it, links and junctions followed; as written when it can't be resolved (a
/// folder that is gone), which the rules still compare without regard to case where the system has none.
pub fn found(path: &Path) -> PathBuf {
  resolved(path).unwrap_or_else(|_| path.to_path_buf())
}

/// The project or git repository `folder` lies in: a listed project's folder, or the nearest folder above it (or
/// itself) with a `.git` entry, which a worktree has as a file.
pub(super) fn repository_of(folder: &Path, projects: &[RegisteredProject]) -> Option<PathBuf> {
  if let Some(project) = projects
    .iter()
    .find(|project| within(folder, &found(&project.repo_root)))
  {
    return Some(project.repo_root.clone());
  }
  folder
    .ancestors()
    .find(|ancestor| ancestor.join(".git").exists())
    .map(Path::to_path_buf)
}

/// Whether `folder`, which Orchester created, is still a folder of its own where it was made: not a link or a
/// junction, and not reached through one, so moving it to the Recycle Bin or Trash takes only what Orchester made.
pub fn still_created_here(folder: &Path) -> bool {
  let real = fs::symlink_metadata(folder).is_ok_and(|found| found.is_dir() && !found.file_type().is_symlink());
  real && resolved(folder).is_ok_and(|found| same_folder(&found, folder))
}
