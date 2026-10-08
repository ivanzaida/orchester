//! The repository's skills Orchester puts into a native agent's prompt while the project has its agent files on:
//! each `SKILL.md` [`RepositorySkills::find`] finds, once, read for its name and description. Like the search, the
//! load refuses rather than leave a skill out: a `SKILL.md` that resolves into the worktree's `.git` or `.orchester`,
//! is not a regular file (never a pipe or device is opened) or cannot be read refuses the agent's start. At most
//! [`MAX_HEADER_BYTES`] of each are read.

use std::{
  collections::HashSet,
  fs::{self, File},
  io::{self, Read},
  path::Path,
};

use thiserror::Error;

use super::{FoundSkill, MAX_HEADER_BYTES, RepositorySkills, RepositorySkillsError, shown_in_worktree};
use crate::{agent_files::frontmatter, native::scope::protected};

/// Why a native agent does not start with the repository's skills. Each names its place in the worktree.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum NativeSkillsError {
  #[error("cannot load the repository's skills: {0}")]
  Search(#[from] RepositorySkillsError),
  #[error("cannot load the repository's skills: `{0}` leads into the worktree's .git or .orchester")]
  Protected(String),
  #[error("cannot load the repository's skills: `{0}` is not a regular file")]
  NotRegular(String),
  #[error("cannot load the repository's skills: `{path}` cannot be read ({error})")]
  Unreadable { path: String, error: String },
}

/// A repository skill as a native agent's prompt lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeSkill {
  /// Its frontmatter's name, else its folder's.
  pub name: String,
  pub description: String,
  /// The first path through the worktree that reaches it, from the worktree with `/`.
  pub path: String,
}

/// The repository's skills of `worktree` for a native agent, each once.
pub fn native_skills(worktree: &Path) -> Result<Vec<NativeSkill>, NativeSkillsError> {
  let skills = RepositorySkills::find(worktree)?;
  let mut loaded = HashSet::new();
  let mut listed = Vec::new();
  for skill in &skills.skills {
    if loaded.insert(skill.resolved.clone()) {
      listed.push(skills.load(skill)?);
    }
  }
  Ok(listed)
}

impl RepositorySkills {
  /// Reads `skill`'s name and description.
  fn load(&self, skill: &FoundSkill) -> Result<NativeSkill, NativeSkillsError> {
    let path = shown_in_worktree(&skill.found, &self.worktree);
    if protected(&skill.resolved, &self.root, true) {
      return Err(NativeSkillsError::Protected(path));
    }
    let unreadable = |error: io::Error| NativeSkillsError::Unreadable {
      path: path.clone(),
      error: error.to_string(),
    };
    if !fs::metadata(&skill.resolved).map_err(unreadable)?.is_file() {
      return Err(NativeSkillsError::NotRegular(path));
    }
    let mut header = Vec::new();
    File::open(&skill.resolved)
      .and_then(|file| file.take(MAX_HEADER_BYTES).read_to_end(&mut header))
      .map_err(unreadable)?;
    let markdown = frontmatter::parse(&String::from_utf8_lossy(&header));
    let folder = skill
      .found
      .parent()
      .and_then(Path::file_name)
      .map(|name| name.to_string_lossy().into_owned());
    Ok(NativeSkill {
      name: markdown.name.or(folder).unwrap_or_default(),
      description: markdown.description.unwrap_or_default(),
      path,
    })
  }
}
