//! The repository's Codex skills, found as Codex finds them (every `SKILL.md` at any depth of
//! [`CODEX_SKILL_FOLDERS`], through links), so that Codex disables each one while the project has its agent files
//! off ([`RepositorySkills::codex_paths`]), and the native agent loads them while they are on ([`native`]).
//!
//! Codex follows links when it loads skills, so a `SKILL.md` this search misses would still be loaded. The search
//! therefore fails closed: a folder or `SKILL.md` that resolves out of the worktree, a folder in the worktree that
//! cannot be read and a search past its bounds each refuse, naming where the cause lies in the worktree, never where
//! a link leads. Every path that reaches a folder is followed, so a link loop runs into the depth bound and refuses.
//!
//! Codex 0.159.2 (Windows, 2026-10-08, `skills/list` of `codex app-server` under a scratch `CODEX_HOME`, no model
//! turn) names a skill reached through a link by its resolved path, and `skills.config` disables it by either path,
//! so both are listed. It passes over hidden entries (`.git`, `.hidden`) and looks 6 folders deep. This search walks
//! hidden folders all the same, in case another version loads them, but never a `.git` entry: a nested repository's
//! `.git` holds hundreds of folders that would only use up the bounds, and Codex skips it as hidden.

pub mod native;

use std::{
  ffi::OsStr,
  fs, io,
  path::{Path, PathBuf},
};

use thiserror::Error;

use crate::{native::scope::protected, permission::canonical, resource::SKILL_FILE};

/// The skills folders of a worktree Codex loads skills from, `.agents/skills` and `.codex/skills`.
pub const CODEX_SKILL_FOLDERS: [[&str; 2]; 2] = [[".agents", "skills"], [".codex", "skills"]];
/// How deep below a skills folder skills are looked for.
pub(crate) const MAX_SEARCH_DEPTH: usize = 8;
/// The most folders one search for skills reads.
pub(crate) const MAX_SEARCHED_FOLDERS: usize = 500;
/// The most bytes of a repository skill's `SKILL.md` read for its name and description.
pub(crate) const MAX_HEADER_BYTES: u64 = 64 * 1024;

/// Why the repository's skills cannot all be found. Each names its place in the worktree, with `/`.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RepositorySkillsError {
  #[error("`{0}` leads out of the worktree")]
  LeadsOut(String),
  #[error("`{path}` cannot be read ({error})")]
  Unreadable { path: String, error: String },
  #[error("`{0}` lies more than {MAX_SEARCH_DEPTH} folders deep in its skills folder")]
  TooDeep(String),
  #[error("the skills folders lead to more than {MAX_SEARCHED_FOLDERS} folders, each path to a folder counted")]
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

/// A `SKILL.md` found, by the path through the worktree, as the caller named it, that reached it, and with its
/// links resolved; both lie within the worktree.
#[derive(Debug)]
struct FoundSkill {
  found: PathBuf,
  resolved: PathBuf,
}

/// The repository's skills one search found, each path that reaches one listed apart.
#[derive(Debug)]
pub struct RepositorySkills {
  /// The worktree as the caller named it.
  worktree: PathBuf,
  /// The worktree with its links resolved.
  root: PathBuf,
  /// Sorted by the path through the worktree.
  skills: Vec<FoundSkill>,
}

impl RepositorySkills {
  /// Every `SKILL.md` under the [`CODEX_SKILL_FOLDERS`] of `worktree`; none from a folder that does not exist.
  pub fn find(worktree: &Path) -> Result<Self, RepositorySkillsError> {
    let root = fs::canonicalize(worktree).map_err(|error| RepositorySkillsError::Unreadable {
      path: ".".to_owned(),
      error: error.to_string(),
    })?;
    let mut search = Search {
      worktree,
      root: canonical::plain(root),
      searched: 0,
      found: Vec::new(),
    };
    for folder in CODEX_SKILL_FOLDERS {
      let folder = folder.iter().fold(worktree.to_path_buf(), |path, name| path.join(name));
      search.folder(&folder, 0)?;
    }
    let Search { root, mut found, .. } = search;
    found.sort_by(|left, right| left.found.cmp(&right.found));
    Ok(Self {
      worktree: worktree.to_path_buf(),
      root,
      skills: found,
    })
  }

  /// Every path Codex may name a skill by, for it to disable each: the path through the worktree and the resolved
  /// one, sorted, each once.
  pub fn codex_paths(&self) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = self
      .skills
      .iter()
      .flat_map(|skill| [skill.found.clone(), skill.resolved.clone()])
      .collect();
    paths.sort();
    paths.dedup();
    paths
  }
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
  /// The folders read below the skills folders, each path to one counted.
  searched: usize,
  found: Vec<FoundSkill>,
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
    if depth > MAX_SEARCH_DEPTH {
      return Err(RepositorySkillsError::TooDeep(shown_in_worktree(found, self.worktree)));
    }
    if depth > 0 {
      self.searched += 1;
      if self.searched > MAX_SEARCHED_FOLDERS {
        return Err(RepositorySkillsError::TooManyFolders);
      }
    }
    let entries = fs::read_dir(&resolved).map_err(|error| self.unreadable(found, &error))?;
    let mut names = Vec::new();
    for entry in entries {
      names.push(entry.map_err(|error| self.unreadable(found, &error))?.file_name());
    }
    names.sort();
    for name in names {
      self.entry(found, &resolved, &name, depth)?;
    }
    Ok(())
  }

  /// Looks at the entry `name` of the folder at `found`, `resolved`, `depth` below its skills folder.
  fn entry(&mut self, found: &Path, resolved: &Path, name: &OsStr, depth: usize) -> Result<(), RepositorySkillsError> {
    // A `.git` entry is a repository's own, which Codex passes over as hidden (see the module's notes).
    if protected(&resolved.join(name), resolved, false) {
      return Ok(());
    }
    let child = found.join(name);
    let target = resolved.join(name);
    let metadata = match fs::metadata(&target) {
      Ok(metadata) => metadata,
      Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
      Err(error) => return Err(self.unreadable(&child, &error)),
    };
    if metadata.is_dir() {
      self.folder(&child, depth + 1)
    } else if name == SKILL_FILE {
      self.document(child, &target)
    } else {
      Ok(())
    }
  }

  /// Collects the `SKILL.md` at `found`, whose links `target` leads through, unless it resolves out of the worktree.
  fn document(&mut self, found: PathBuf, target: &Path) -> Result<(), RepositorySkillsError> {
    let resolved = fs::canonicalize(target).map_err(|error| self.unreadable(&found, &error))?;
    let resolved = canonical::plain(resolved);
    if !resolved.starts_with(&self.root) {
      return Err(RepositorySkillsError::LeadsOut(shown_in_worktree(
        &found,
        self.worktree,
      )));
    }
    self.found.push(FoundSkill { found, resolved });
    Ok(())
  }

  fn unreadable(&self, found: &Path, error: &io::Error) -> RepositorySkillsError {
    RepositorySkillsError::Unreadable {
      path: shown_in_worktree(found, self.worktree),
      error: error.to_string(),
    }
  }
}
