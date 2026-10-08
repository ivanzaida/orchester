//! The repository's Codex skills, found as Codex finds them (every `SKILL.md` at any depth of
//! [`CODEX_SKILL_FOLDERS`], through links), so that Codex disables each one while the project has its agent files
//! off, and the native agent loads them while they are on.
//!
//! Codex follows links when it loads skills, so a `SKILL.md` this search misses would still be loaded. The search
//! therefore fails closed: a folder or `SKILL.md` that resolves out of the worktree, a folder in the worktree that
//! cannot be read and a search past its bounds each refuse, naming where the cause lies in the worktree, never where
//! a link leads. A link loop within the worktree reads each folder once.

use std::{
  collections::HashSet,
  fs, io,
  path::{Path, PathBuf},
};

use thiserror::Error;

use crate::{permission::canonical, resource::SKILL_FILE};

/// The skills folders of a worktree Codex loads skills from, `.agents/skills` and `.codex/skills`.
pub const CODEX_SKILL_FOLDERS: [[&str; 2]; 2] = [[".agents", "skills"], [".codex", "skills"]];
/// How deep below a skills folder skills are looked for.
pub(crate) const MAX_SEARCH_DEPTH: usize = 8;
/// The most folders one search for skills reads.
pub(crate) const MAX_SEARCHED_FOLDERS: usize = 500;

/// Why the repository's skills cannot all be found. Each names its place in the worktree, with `/`.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RepositorySkillsError {
  #[error("`{0}` leads out of the worktree")]
  LeadsOut(String),
  #[error("`{path}` cannot be read ({error})")]
  Unreadable { path: String, error: String },
  #[error("`{0}` lies more than {MAX_SEARCH_DEPTH} folders deep in its skills folder")]
  TooDeep(String),
  #[error("the skills folders hold more than {MAX_SEARCHED_FOLDERS} folders")]
  TooManyFolders,
}

impl RepositorySkillsError {
  /// Why a Codex agent does not start while the project has its agent files off: Codex could load a skill
  /// Orchester did not find to disable.
  pub fn refusal_while_off(&self) -> String {
    let (consequence, remedy) = match self {
      Self::LeadsOut(_) => ("Codex could load skills from it", "Remove the link"),
      Self::Unreadable { .. } => (
        "Orchester cannot disable the skills Codex could load from it",
        "Make it readable",
      ),
      Self::TooDeep(_) | Self::TooManyFolders => (
        "Codex could load skills there that Orchester does not look for",
        "Make the skills folders smaller",
      ),
    };
    format!(
      "Repository agent files are off, but {self}, so {consequence}. {remedy} or turn the project's Repository \
       agent files on."
    )
  }
}

/// Every `SKILL.md` under the [`CODEX_SKILL_FOLDERS`] of `worktree`, each by its path through `worktree` as Codex
/// finds it, sorted; none from a folder that does not exist.
pub fn repository_skills(worktree: &Path) -> Result<Vec<PathBuf>, RepositorySkillsError> {
  let root = fs::canonicalize(worktree).map_err(|error| RepositorySkillsError::Unreadable {
    path: ".".to_owned(),
    error: error.to_string(),
  })?;
  let mut search = Search {
    worktree,
    root: canonical::plain(root),
    visited: HashSet::new(),
    found: Vec::new(),
  };
  for folder in CODEX_SKILL_FOLDERS {
    let folder = folder.iter().fold(worktree.to_path_buf(), |path, name| path.join(name));
    search.folder(&folder, 0)?;
  }
  search.found.sort();
  Ok(search.found)
}

/// `found`, a path through `worktree`, from the worktree with `/` and without control characters: where it lies in
/// the worktree, never where a link leads.
pub(crate) fn shown_in_worktree(found: &Path, worktree: &Path) -> String {
  let Ok(relative) = found.strip_prefix(worktree) else {
    return "a skills folder".to_owned();
  };
  relative
    .to_string_lossy()
    .chars()
    .map(|c| match c {
      '\\' => '/',
      c if c.is_control() => '?',
      c => c,
    })
    .collect()
}

/// One search of a worktree's skills folders.
struct Search<'a> {
  /// The worktree as the caller names it, which every path found starts with.
  worktree: &'a Path,
  /// The worktree with its links resolved.
  root: PathBuf,
  /// The folders read, with their links resolved: a link loop reads each once.
  visited: HashSet<PathBuf>,
  found: Vec<PathBuf>,
}

impl Search<'_> {
  /// Collects the `SKILL.md` files of the folder at `found`, `depth` below its skills folder.
  fn folder(&mut self, found: &Path, depth: usize) -> Result<(), RepositorySkillsError> {
    let resolved = match fs::canonicalize(found) {
      Ok(resolved) => canonical::plain(resolved),
      // A missing skills folder, or a link to nothing, holds no skill.
      Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
      Err(error) => return Err(self.unreadable(found, &error)),
    };
    if !resolved.starts_with(&self.root) {
      return Err(RepositorySkillsError::LeadsOut(shown_in_worktree(found, self.worktree)));
    }
    if !self.visited.insert(resolved.clone()) {
      return Ok(());
    }
    if depth > MAX_SEARCH_DEPTH {
      return Err(RepositorySkillsError::TooDeep(shown_in_worktree(found, self.worktree)));
    }
    if self.visited.len() > MAX_SEARCHED_FOLDERS {
      return Err(RepositorySkillsError::TooManyFolders);
    }
    let entries = fs::read_dir(&resolved).map_err(|error| self.unreadable(found, &error))?;
    for entry in entries {
      let name = entry.map_err(|error| self.unreadable(found, &error))?.file_name();
      let child = found.join(&name);
      let target = resolved.join(&name);
      let metadata = match fs::metadata(&target) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
        Err(error) => return Err(self.unreadable(&child, &error)),
      };
      if metadata.is_dir() {
        self.folder(&child, depth + 1)?;
      } else if name == SKILL_FILE {
        self.document(child, &target)?;
      }
    }
    Ok(())
  }

  /// Collects the `SKILL.md` at `found`, whose links `target` leads through, unless it resolves out of the worktree.
  fn document(&mut self, found: PathBuf, target: &Path) -> Result<(), RepositorySkillsError> {
    let resolved = fs::canonicalize(target).map_err(|error| self.unreadable(&found, &error))?;
    if !canonical::plain(resolved).starts_with(&self.root) {
      return Err(RepositorySkillsError::LeadsOut(shown_in_worktree(
        &found,
        self.worktree,
      )));
    }
    self.found.push(found);
    Ok(())
  }

  fn unreadable(&self, found: &Path, error: &io::Error) -> RepositorySkillsError {
    RepositorySkillsError::Unreadable {
      path: shown_in_worktree(found, self.worktree),
      error: error.to_string(),
    }
  }
}
