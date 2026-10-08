//! `list_skills` and `read_skill`: the skills one step execution offers its agent, read by name through
//! Orchester's tools rather than by path (ORC-15, `crate::resource::skill`).
//!
//! - Orchester's skills are the ones the runner placed for this step execution: the run's pinned versions that reach
//!   the agent, and Orchester's internal ones, in `<repo>/.orchester/skills/<execution>/<slug>/`. That folder is
//!   Orchester's state, which `read_outside_file` never reads, so these tools are how an agent reads them. A skill
//!   reads its own `SKILL.md` and the reference files its version lists, nothing else.
//! - The repository's own skills (the `SKILL.md` folders of the worktree's `.agents/skills`, `.codex/skills` and
//!   `.claude/skills`) are offered only while the project lets agents load the repository's agent files
//!   (`crate::agent_files::RepositoryAgentFiles`). Their reference files are the files of their folder in the worktree,
//!   but a folder holding a skill of its own, a `.git` entry and anything a link leads out of the folder.
//! - Each name is offered once: Orchester's skills first, then the repository's in path order. A later skill with a
//!   name already offered, or a repository skill whose folder leads out of the worktree, is listed as left out.
//! - A read answers a text file's numbered lines within the limits of `read_outside_file` ([`super::MAX_READ_BYTES`]).

use std::{
  fmt::Write,
  fs,
  io::{self, Read},
  path::{Path, PathBuf},
};

use thiserror::Error;

use super::{OutsideReadError, ToolCallError, ToolResult, outside::read_lines};
use crate::{
  agent_files::frontmatter,
  native::scope::protected,
  permission::canonical,
  resource::{SKILL_FILE, SkillFilePath},
};

/// The most reference files `list_skills` names for one repository skill.
const MAX_LISTED_FILES: usize = 200;
/// How deep below its folder a repository skill's reference files are looked for.
const MAX_FILE_DEPTH: usize = 8;
/// The most bytes of a repository skill's `SKILL.md` read for its name and description.
const MAX_HEADER_BYTES: u64 = 64 * 1024;

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

/// Why a skill cannot be offered when the agent starts.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("cannot read the skill at {}: {kind}", .path.display())]
pub struct SkillOfferError {
  path: PathBuf,
  kind: io::ErrorKind,
}

impl SkillOfferError {
  fn of(path: &Path) -> impl FnOnce(io::Error) -> Self + '_ {
    move |error| Self {
      path: path.to_path_buf(),
      kind: error.kind(),
    }
  }
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
    let folder = document.parent().unwrap_or(document);
    let folder = fs::canonicalize(folder)
      .map(canonical::plain)
      .map_err(SkillOfferError::of(folder))?;
    self.offer(
      OfferedSkill {
        name: name.to_owned(),
        description: description.to_owned(),
        source: SkillSource::Orchester,
        folder,
        listed: Some(files.to_vec()),
      },
      name,
    );
    Ok(())
  }

  /// Offers the repository skill whose `SKILL.md` is `document`, found in `worktree`: its name and description
  /// from its front matter, the name of its folder when it has none. One whose folder leads out of the worktree, or
  /// into a protected folder of it, is left out.
  pub fn add_repository(&mut self, document: &Path, worktree: &Path) -> Result<(), SkillOfferError> {
    let shown = document
      .strip_prefix(worktree)
      .unwrap_or(document)
      .to_string_lossy()
      .replace('\\', "/");
    let root = fs::canonicalize(worktree)
      .map(canonical::plain)
      .map_err(SkillOfferError::of(worktree))?;
    let found = document.parent().unwrap_or(document);
    let folder = fs::canonicalize(found)
      .map(canonical::plain)
      .map_err(SkillOfferError::of(found))?;
    if !folder.starts_with(&root) || protected(&folder, &root, true) {
      self
        .left_out
        .push(format!("{shown}: its folder leads out of the worktree"));
      return Ok(());
    }
    let mut header = Vec::new();
    fs::File::open(document)
      .and_then(|file| file.take(MAX_HEADER_BYTES).read_to_end(&mut header))
      .map_err(SkillOfferError::of(document))?;
    let markdown = frontmatter::parse(&String::from_utf8_lossy(&header));
    let name = markdown
      .name
      .or_else(|| found.file_name().map(|name| name.to_string_lossy().into_owned()))
      .unwrap_or_default();
    self.offer(
      OfferedSkill {
        name,
        description: markdown.description.unwrap_or_default(),
        source: SkillSource::Repository,
        folder,
        listed: None,
      },
      &shown,
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
      let description = skill.description.split_whitespace().collect::<Vec<_>>().join(" ");
      let files = match &skill.listed {
        Some(listed) => listed.iter().map(|file| file.as_str().to_owned()).collect(),
        None => folder_files(&skill.folder),
      };
      let files = if files.is_empty() {
        "none".to_owned()
      } else {
        files.join(", ")
      };
      // Writing to a String cannot fail.
      let _ = writeln!(text, "- {} ({}): {description}", skill.name, skill.source.as_str());
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
    if !belongs(&resolved, &skill.folder) {
      return Err(not_in_skill());
    }
    Ok(read_lines(resolved, offset, limit)?)
  }
}

/// Whether `path`, resolved, is a file of the skill in `folder`: inside it, in no `.git` entry and in no folder
/// below it that holds a skill of its own.
fn belongs(path: &Path, folder: &Path) -> bool {
  if !path.starts_with(folder) || protected(path, folder, false) {
    return false;
  }
  path
    .ancestors()
    .skip(1)
    .take_while(|ancestor| *ancestor != folder)
    .all(|ancestor| !ancestor.join(SKILL_FILE).exists())
}

/// The reference files of the repository skill in `folder`, as paths from it with `/`: at most
/// [`MAX_LISTED_FILES`], with a last entry saying when there are more.
fn folder_files(folder: &Path) -> Vec<String> {
  let mut found = Vec::new();
  collect_files(folder, folder, 0, &mut found);
  found.sort();
  if found.len() > MAX_LISTED_FILES {
    let more = found.len() - MAX_LISTED_FILES;
    found.truncate(MAX_LISTED_FILES);
    found.push(format!("[{more} more not listed]"));
  }
  found
}

/// Collects the files of `current`, below the skill's `folder`, that [`belongs`] lets the skill read. An entry that
/// cannot be read or resolved is left out of the list: the list only helps, and `read_skill` answers for each file
/// with its own error.
fn collect_files(folder: &Path, current: &Path, depth: usize, found: &mut Vec<String>) {
  let Ok(entries) = fs::read_dir(current) else {
    return;
  };
  for entry in entries.flatten() {
    let path = entry.path();
    let Ok(resolved) = fs::canonicalize(&path).map(canonical::plain) else {
      continue;
    };
    if resolved.is_dir() {
      let inside = resolved.starts_with(folder) && !protected(&resolved, folder, false);
      if inside && depth < MAX_FILE_DEPTH && !resolved.join(SKILL_FILE).exists() {
        collect_files(folder, &path, depth + 1, found);
      }
      continue;
    }
    let Ok(relative) = path.strip_prefix(folder) else {
      continue;
    };
    let relative = relative.to_string_lossy().replace('\\', "/");
    if belongs(&resolved, folder) && SkillFilePath::new(&relative).is_ok() {
      found.push(relative);
    }
  }
}
