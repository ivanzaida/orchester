//! Skills (ORC-15, `docs/design/orchester-workspace.md`, "Instructions"): know-how an agent loads
//! when a task needs it. A skill is a Markdown body (its `SKILL.md`), a description an agent reads
//! to decide whether to load it, reference files beside it, and the scope that decides which agents
//! get it (the project, a role, a pipeline or a step, as for instructions). It is versioned like
//! every resource; a run pins the published versions in its scope. An agent is told each skill's
//! name, description and where it was placed, and reads the body and files itself when it needs
//! them. Skills never hold secrets: their text is sent as written, without variables.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
  instruction::{Exclusion, InstructionScope, InstructionTarget},
  lifecycle::Slug,
};
use crate::blob::ContentHash;

/// The file name of a skill's body, as agents and the CLIs know it.
pub const SKILL_FILE: &str = "SKILL.md";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SkillError {
  #[error(
    "a reference file path is 1–{max} characters of `/`-separated names, without `.`, `..`, a drive or characters \
     Windows does not allow, and not {SKILL_FILE}; got `{0}`",
    max = SkillFilePath::MAX_CHARS
  )]
  InvalidPath(String),
  #[error("the skill has two reference files at `{0}`")]
  DuplicateFile(String),
}

/// Where a reference file sits in its skill's folder, e.g. `references/checks.md`: relative, with
/// `/` between names, and a valid file path on Windows and macOS.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SkillFilePath(String);

impl SkillFilePath {
  pub const MAX_CHARS: usize = 200;

  pub fn new(value: &str) -> Result<Self, SkillError> {
    let invalid = || SkillError::InvalidPath(value.to_owned());
    if value.is_empty() || value.chars().count() > Self::MAX_CHARS || value == SKILL_FILE {
      return Err(invalid());
    }
    if value.split('/').all(valid_name) {
      Ok(Self(value.to_owned()))
    } else {
      Err(invalid())
    }
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }

  /// The names from the skill's folder down to the file.
  pub fn segments(&self) -> impl Iterator<Item = &str> {
    self.0.split('/')
  }
}

/// A file or folder name both platforms accept and that stays inside the skill's folder.
fn valid_name(name: &str) -> bool {
  const FORBIDDEN: &[char] = &['\\', ':', '*', '?', '"', '<', '>', '|'];
  !name.is_empty()
    && name != "."
    && name != ".."
    && !name.ends_with(['.', ' '])
    && !name.chars().any(|c| c.is_control() || FORBIDDEN.contains(&c))
}

impl TryFrom<String> for SkillFilePath {
  type Error = SkillError;

  fn try_from(value: String) -> Result<Self, SkillError> {
    Self::new(&value)
  }
}

impl From<SkillFilePath> for String {
  fn from(path: SkillFilePath) -> Self {
    path.0
  }
}

/// A reference file of a skill: its path in the skill's folder and its content, stored as a project
/// blob.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillFile {
  pub path: SkillFilePath,
  pub content: ContentHash,
  /// Bytes of the content.
  pub size: u64,
}

/// A project skill. A draft may have a blank description and body; publishing requires both
/// ([`crate::resource::ResourceBody`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "SkillFields")]
pub struct Skill {
  description: String,
  body: String,
  files: Vec<SkillFile>,
  scope: InstructionScope,
}

#[derive(Deserialize)]
struct SkillFields {
  description: String,
  body: String,
  files: Vec<SkillFile>,
  scope: InstructionScope,
}

impl TryFrom<SkillFields> for Skill {
  type Error = SkillError;

  fn try_from(fields: SkillFields) -> Result<Self, SkillError> {
    Self::new(fields.description, fields.body, fields.files, fields.scope)
  }
}

impl Skill {
  /// A skill whose reference files each have their own path; they are kept in path order.
  pub fn new(
    description: String,
    body: String,
    mut files: Vec<SkillFile>,
    scope: InstructionScope,
  ) -> Result<Self, SkillError> {
    files.sort_by(|left, right| left.path.cmp(&right.path));
    if let Some(pair) = files.windows(2).find(|pair| pair[0].path == pair[1].path) {
      return Err(SkillError::DuplicateFile(pair[0].path.as_str().to_owned()));
    }
    Ok(Self {
      description,
      body,
      files,
      scope,
    })
  }

  /// What an agent reads to decide whether to load the skill.
  pub fn description(&self) -> &str {
    &self.description
  }

  /// The Markdown of its `SKILL.md`, without front matter.
  pub fn body(&self) -> &str {
    &self.body
  }

  /// Reference files, in path order.
  pub fn files(&self) -> &[SkillFile] {
    &self.files
  }

  pub fn scope(&self) -> &InstructionScope {
    &self.scope
  }

  /// Whether `target`'s agent gets the skill.
  pub fn reaches(&self, target: InstructionTarget<'_>) -> Result<(), Exclusion> {
    self.scope.admits(target)
  }

  /// The `SKILL.md` an agent reads: front matter with the skill's `name` and `description`, then
  /// the body. Both values are written as double-quoted YAML (a JSON string is one), so any text
  /// keeps its meaning.
  pub fn document(&self, name: &Slug) -> String {
    let quoted = |text: &str| serde_json::to_string(text).expect("a string serializes to JSON");
    format!(
      "---\nname: {}\ndescription: {}\n---\n\n{}\n",
      quoted(name.as_str()),
      quoted(self.description.trim()),
      self.body.trim()
    )
  }
}
