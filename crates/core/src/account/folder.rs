//! Where an account's folder may be. Orchester creates new ones in its own application data; a folder the person
//! brings must stay outside every project, worktree and repository, so a sign-in can't be committed, must not be
//! another account's, and must lie where agents' reads already ask first: the home folder's dot folders or the
//! applications' data (`AppData`, `Library`; the design note for open decision 4).

use std::path::{Component, Path, PathBuf};

use super::AgentAccount;
use crate::id::AgentAccountId;

/// Why a folder can't be an account's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FolderProblem {
  /// Not a full path, or one with `.` or `..` in it.
  NotAbsolute,
  /// Inside the repository or project at this root.
  InsideRepository(PathBuf),
  /// Another account's folder, or the CLI's own folder, which is Default's; or a folder inside one or holding one,
  /// which removing either account would take along.
  Taken(AgentAccountId),
  /// Neither in a dot folder of the home folder nor in the applications' data.
  OutsideCredentialPlaces,
}

/// The name of a folder Orchester creates for an account of `program` named `name`: `<program>-<name>` in lower
/// case with anything but letters and digits as one `-`, numbered while `taken` says the name is used.
pub fn account_folder_name(program: &str, name: &str, taken: impl Fn(&str) -> bool) -> String {
  let mut slug = String::new();
  for character in name.chars().flat_map(char::to_lowercase) {
    if character.is_alphanumeric() {
      slug.push(character);
    } else if !slug.is_empty() && !slug.ends_with('-') {
      slug.push('-');
    }
  }
  let slug = slug.trim_end_matches('-');
  let base = if slug.is_empty() {
    program.to_owned()
  } else {
    format!("{program}-{slug}")
  };
  (1..)
    .map(|number| {
      if number == 1 {
        base.clone()
      } else {
        format!("{base}-{number}")
      }
    })
    .find(|candidate| !taken(candidate))
    .expect("an unbounded range always has a free name")
}

/// What is wrong with `folder` as a brought folder of an account of `accounts`' CLI, if anything. `repository` is
/// the repository or project folder that holds it, as the caller found on disk; `own` the CLI's own folder; `home`
/// the home folder. `found` gives a path as the file system resolves it ([`resolved`]); every path is compared as
/// found, `folder` itself only once it is a plain full path.
pub fn brought_folder_problem(
  folder: &Path,
  repository: Option<&Path>,
  (accounts, own): (&[AgentAccount], Option<&Path>),
  home: Option<&Path>,
  found: &dyn Fn(&Path) -> PathBuf,
) -> Option<FolderProblem> {
  if !plain_full_path(folder) {
    return Some(FolderProblem::NotAbsolute);
  }
  if let Some(repository) = repository {
    return Some(FolderProblem::InsideRepository(repository.to_path_buf()));
  }
  let folder = found(folder);
  if let Some(account) = accounts.iter().find(|account| {
    let path = account
      .folder()
      .path()
      .or(if account.is_default() { own } else { None });
    path
      .map(found)
      .is_some_and(|path| within(&folder, &path) || within(&path, &folder))
  }) {
    return Some(FolderProblem::Taken(account.id()));
  }
  if !home.map(found).is_some_and(|home| in_credential_place(&folder, &home)) {
    return Some(FolderProblem::OutsideCredentialPlaces);
  }
  None
}

/// Whether `folder` is a full path without `.` or `..` in it.
pub fn plain_full_path(folder: &Path) -> bool {
  folder.is_absolute()
    && folder
      .components()
      .all(|part| !matches!(part, Component::CurDir | Component::ParentDir))
}

/// Whether two folders are one, compared as Windows compares names (without regard to case) on Windows.
pub fn same_folder(left: &Path, right: &Path) -> bool {
  parts(left) == parts(right)
}

/// Whether `folder` is `root` or lies inside it, compared as [`same_folder`] compares.
pub fn within(folder: &Path, root: &Path) -> bool {
  parts(folder).starts_with(&parts(root))
}

/// `folder` as the file system has it, links and junctions resolved as far as it exists, for the rules above to
/// compare: two spellings of one folder become one path.
pub fn resolved(folder: &Path) -> std::io::Result<PathBuf> {
  crate::permission::canonical::existing(folder)
    .map(|found| found.path)
    .map_err(|failure| failure.error)
}

fn parts(path: &Path) -> Vec<String> {
  path
    .components()
    .filter(|part| !matches!(part, Component::CurDir))
    .map(|part| {
      let text = part.as_os_str().to_string_lossy();
      if cfg!(windows) {
        text.to_lowercase()
      } else {
        text.into_owned()
      }
    })
    .collect()
}

/// Whether `folder` lies in a dot folder of `home` or in its applications' data.
fn in_credential_place(folder: &Path, home: &Path) -> bool {
  if !within(folder, home) {
    return false;
  }
  let skip = parts(home).len();
  match parts(folder).get(skip) {
    Some(first) => {
      first.starts_with('.') || first.eq_ignore_ascii_case("appdata") || first.eq_ignore_ascii_case("library")
    }
    None => false,
  }
}
