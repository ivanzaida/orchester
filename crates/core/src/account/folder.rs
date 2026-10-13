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
  NotAbsolute,
  /// Inside the repository or project at this root.
  InsideRepository(PathBuf),
  /// Another account's folder, or the CLI's own folder, which is Default's.
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
/// the home folder.
pub fn brought_folder_problem(
  folder: &Path,
  repository: Option<&Path>,
  accounts: &[AgentAccount],
  own: Option<&Path>,
  home: Option<&Path>,
) -> Option<FolderProblem> {
  if !folder.is_absolute() {
    return Some(FolderProblem::NotAbsolute);
  }
  if let Some(repository) = repository {
    return Some(FolderProblem::InsideRepository(repository.to_path_buf()));
  }
  if let Some(account) = accounts.iter().find(|account| {
    let path = account
      .folder()
      .path()
      .or(if account.is_default() { own } else { None });
    path.is_some_and(|path| same_folder(path, folder))
  }) {
    return Some(FolderProblem::Taken(account.id()));
  }
  if !home.is_some_and(|home| in_credential_place(folder, home)) {
    return Some(FolderProblem::OutsideCredentialPlaces);
  }
  None
}

/// Whether two folders are one, compared as Windows compares names (without regard to case) on Windows.
pub fn same_folder(left: &Path, right: &Path) -> bool {
  let parts = |path: &Path| -> Vec<String> {
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
  };
  parts(left) == parts(right)
}

/// Whether `folder` lies in a dot folder of `home` or in its applications' data.
fn in_credential_place(folder: &Path, home: &Path) -> bool {
  let Ok(rest) = folder.strip_prefix(home) else {
    return false;
  };
  let mut names = rest
    .components()
    .map(|part| part.as_os_str().to_string_lossy().to_lowercase());
  match names.next() {
    Some(first) => first.starts_with('.') || first == "appdata" || first == "library",
    None => false,
  }
}
