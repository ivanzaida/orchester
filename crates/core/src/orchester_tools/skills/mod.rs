//! `list_skills` and `read_skill`: the skills one step execution offers its agent, read by name through
//! Orchester's tools rather than by path (ORC-15, `crate::resource::skill`).
//!
//! - Orchester's skills are the ones the runner placed for this step execution: the run's pinned versions that reach
//!   the agent, and Orchester's internal ones, in `<repo>/.orchester/skills/<execution>/<slug>/`. That folder is
//!   Orchester's state, which `read_outside_file` never reads, so these tools are how an agent reads them. A skill
//!   reads its own `SKILL.md` and the reference files its version lists, nothing else.
//! - The repository's own skills are the ones the agent's CLI itself loads while the project lets it load the
//!   repository's agent files (`crate::agent_files::RepositoryAgentFiles`): the runner names their folders, and they
//!   are found within the worktree only ([`repository`]). Their reference files are the files of their folder, but a
//!   folder holding a skill of its own, a `.git` entry, the worktree's `.orchester` and anything a link leads out of
//!   the folder or the worktree to.
//! - Each name is offered once: Orchester's skills first, then the repository's in path order. A skill not offered is
//!   listed as left out, by where it lies in the worktree, never by a path outside it.
//! - A read answers a regular text file's numbered lines within the limits of `read_outside_file`
//!   ([`super::MAX_READ_BYTES`]). Listing and reading apply the same rule of what belongs to a skill ([`belongs`]).

mod repository;

use std::{
  fmt::Write,
  fs, io,
  path::{Path, PathBuf},
};

use thiserror::Error;

use super::{OutsideReadError, ToolCallError, ToolResult, outside::read_lines};
use crate::{
  native::scope::protected,
  permission::canonical,
  resource::{SKILL_FILE, SkillFilePath},
};

/// Where a skill comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkillSource {
  /// Orchester's Skills library, or Orchester's own internal skills.
  Orchester,
  /// The repository's own skill folders.
  Repository,
}

impl SkillSource {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Orchester => "orchester",
      Self::Repository => "repository",
    }
  }
}

/// A call of `list_skills` or `read_skill`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SkillCall {
  List,
  /// Lines `offset` (from 1) to `offset + limit - 1` of skill `name`'s `SKILL.md`, or of its reference `file`.
  Read {
    name: String,
    file: Option<SkillFilePath>,
    offset: u32,
    limit: u32,
  },
}

/// Why a skill's file is not read. The message is the error result the agent gets.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum SkillReadError {
  #[error("this step has no skill named `{0}`: list_skills names its skills")]
  UnknownSkill(String),
  #[error("`{file}` is not a reference file of the skill `{skill}`: list_skills lists its files")]
  NotInSkill { skill: String, file: String },
  #[error(transparent)]
  File(#[from] OutsideReadError),
}

/// Why a skill Orchester placed cannot be offered when the agent starts.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("cannot read the skill at {}: {kind}", .path.display())]
pub struct SkillOfferError {
  path: PathBuf,
  kind: io::ErrorKind,
}

/// A skill one step offers.
#[derive(Clone, Debug)]
struct OfferedSkill {
  name: String,
  description: String,
  source: SkillSource,
  /// The folder holding its `SKILL.md`, with links resolved.
  folder: PathBuf,
  /// The reference files its version lists, for Orchester's skill; a repository skill's are its folder's files.
  listed: Option<Vec<SkillFilePath>>,
  /// The worktree, with links resolved, for a repository skill: nothing of the worktree's `.git` or `.orchester`
  /// is the skill's, whatever its folder resolves to.
  worktree: Option<PathBuf>,
}

/// The skills one step execution offers its agent through `list_skills` and `read_skill`.
#[derive(Clone, Debug, Default)]
pub struct StepSkills {
  skills: Vec<OfferedSkill>,
  /// Skills not offered, each with why.
  left_out: Vec<String>,
}

impl StepSkills {
  /// Whether the step offers no skill: it gets neither tool.
  pub fn is_empty(&self) -> bool {
    self.skills.is_empty()
  }

  /// Offers Orchester's skill `name`, placed for this step execution with its `SKILL.md` at `document` and the
  /// reference files `files` beside it.
  pub fn add_placed(
    &mut self,
    (name, description): (&str, &str),
    document: &Path,
    files: &[SkillFilePath],
  ) -> Result<(), SkillOfferError> {
    let found = document.parent().unwrap_or(document);
    let folder = fs::canonicalize(found)
      .map(canonical::plain)
      .map_err(|error| SkillOfferError {
        path: found.to_path_buf(),
        kind: error.kind(),
      })?;
    self.offer(
      OfferedSkill {
        name: name.to_owned(),
        description: description.to_owned(),
        source: SkillSource::Orchester,
        folder,
        listed: Some(files.to_vec()),
        worktree: None,
      },
      name,
    );
    Ok(())
  }

  /// Offers `skill`, shown as `shown`, unless a skill offered before has its name.
  fn offer(&mut self, skill: OfferedSkill, shown: &str) {
    if self.skills.iter().any(|offered| offered.name == skill.name) {
      self
        .left_out
        .push(format!("{shown}: the name `{}` is another skill's", skill.name));
    } else {
      self.skills.push(skill);
    }
  }

  /// The answer to `call`. It blocks on the file system.
  pub fn run(&self, call: &SkillCall) -> ToolResult {
    match call {
      SkillCall::List => ToolResult::shown(self.list()),
      SkillCall::Read {
        name,
        file,
        offset,
        limit,
      } => match self.read(name, file.as_ref(), (*offset, *limit)) {
        Ok(text) => ToolResult::shown(text),
        Err(error) => ToolResult::refused(&ToolCallError::Skill(error)),
      },
    }
  }

  /// Each skill with its source, description and reference files, then the skills left out.
  fn list(&self) -> String {
    let mut text = String::from("Read a skill with read_skill and its name; a reference file with its file too.\n");
    for skill in &self.skills {
      let files = match &skill.listed {
        Some(listed) => listed.iter().map(|file| file.as_str().to_owned()).collect(),
        None => repository::folder_files(skill),
      };
      let files = if files.is_empty() {
        "none".to_owned()
      } else {
        files.join(", ")
      };
      // Writing to a String cannot fail.
      let _ = writeln!(
        text,
        "- {} ({}): {}",
        skill.name,
        skill.source.as_str(),
        one_line(&skill.description)
      );
      let _ = writeln!(text, "  reference files: {files}");
    }
    if !self.left_out.is_empty() {
      text.push_str("Left out:\n");
      for line in &self.left_out {
        let _ = writeln!(text, "- {line}");
      }
    }
    text
  }

  /// Lines `offset` to `offset + limit - 1` of skill `name`'s `SKILL.md`, or of its reference `file`.
  fn read(
    &self,
    name: &str,
    file: Option<&SkillFilePath>,
    (offset, limit): (u32, u32),
  ) -> Result<String, SkillReadError> {
    let skill = self
      .skills
      .iter()
      .find(|skill| skill.name == name)
      .ok_or_else(|| SkillReadError::UnknownSkill(name.to_owned()))?;
    let not_in_skill = || SkillReadError::NotInSkill {
      skill: skill.name.clone(),
      file: file.map_or(SKILL_FILE, SkillFilePath::as_str).to_owned(),
    };
    let path = match file {
      None => skill.folder.join(SKILL_FILE),
      Some(file) => {
        if skill.listed.as_ref().is_some_and(|listed| !listed.contains(file)) {
          return Err(not_in_skill());
        }
        file.segments().fold(skill.folder.clone(), |path, name| path.join(name))
      }
    };
    let resolved = match fs::canonicalize(&path) {
      Ok(resolved) => canonical::plain(resolved),
      Err(error) if error.kind() == io::ErrorKind::NotFound => return Err(not_in_skill()),
      Err(error) => {
        return Err(SkillReadError::File(OutsideReadError::Unreadable {
          path,
          kind: error.kind(),
        }));
      }
    };
    // Only a regular file is opened: opening a pipe would wait for a writer.
    if !belongs(&resolved, skill) || !is_regular_file(&resolved) {
      return Err(not_in_skill());
    }
    Ok(read_lines(resolved, offset, limit)?)
  }
}

/// Whether `path`, resolved, is a file of `skill`: inside its folder, in no `.git` entry, in no folder below it that
/// holds a skill of its own, and, for a repository skill, inside the worktree and not in its `.orchester`.
fn belongs(path: &Path, skill: &OfferedSkill) -> bool {
  let folder = &skill.folder;
  let in_worktree = skill
    .worktree
    .as_ref()
    .is_none_or(|worktree| path.starts_with(worktree) && !protected(path, worktree, true));
  if !in_worktree || !path.starts_with(folder) || protected(path, folder, false) {
    return false;
  }
  path
    .ancestors()
    .skip(1)
    .take_while(|ancestor| *ancestor != folder.as_path())
    .all(|ancestor| !ancestor.join(SKILL_FILE).exists())
}

/// Whether `path` is a regular file, as its links lead.
fn is_regular_file(path: &Path) -> bool {
  fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
}

/// `text` on one line: its whitespace runs as one space, without control characters, so it cannot start a line of
/// its own in a list.
fn one_line(text: &str) -> String {
  text
    .split_whitespace()
    .collect::<Vec<_>>()
    .join(" ")
    .chars()
    .filter(|c| !c.is_control())
    .collect()
}
