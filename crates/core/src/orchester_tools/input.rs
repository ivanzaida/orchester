//! The typed input of each tool, read strictly from a call's arguments (unknown fields refused), then
//! checked into values that cannot carry what the program would read as something else: a path that is
//! relative and stays in the worktree, a revision that is no option, a message that goes to the
//! program's standard input rather than its arguments.

use std::{fmt, num::NonZeroU32};

use serde::Deserialize;
use serde_json::Value;

use super::{FindSymbol, OrchesterTool, SkillCall, StepTools, ToolCallError};
use crate::{
  native::tools::{ToolInput, ToolInputError, parse_call},
  resource::{SKILL_FILE, SkillFilePath},
};

/// Commits `git_log` lists when the call gives no `count`.
pub(super) const DEFAULT_LOG_COUNT: u32 = 20;
pub(super) const MAX_LOG_COUNT: u32 = 200;
/// Paths one `git_add` call may stage.
pub(super) const MAX_ADD_PATHS: usize = 100;
/// Lines `read_outside_file` returns when the call gives no `limit`.
pub(super) const DEFAULT_READ_LINES: u32 = 2000;
pub(super) const MAX_READ_LINES: u32 = 10_000;
/// The longest path, revision or project tool name a call may give, in characters.
const MAX_TEXT_CHARS: usize = 4096;
const MAX_REVISION_CHARS: usize = 256;
const MAX_MESSAGE_CHARS: usize = 20_000;
/// The longest skill name a call may give, in characters.
const MAX_SKILL_NAME_CHARS: usize = 200;

/// A file or folder relative to the worktree root: not absolute, no `..`, no `:` (a Windows drive or
/// stream, or git's pathspec magic), no control character.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoPath(String);

impl RepoPath {
  pub fn new(value: &str) -> Result<Self, String> {
    if value.is_empty() {
      return Err("a path must not be empty".to_owned());
    }
    if value.chars().count() > MAX_TEXT_CHARS || value.chars().any(char::is_control) {
      return Err(format!("`{value}` is not a path"));
    }
    if value.contains(':') || value.starts_with(['/', '\\']) {
      return Err(format!("`{value}` is not relative to the worktree root"));
    }
    if value.split(['/', '\\']).any(|part| part == "..") {
      return Err(format!("`{value}` leaves the worktree"));
    }
    Ok(Self(value.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

/// A revision git resolves: no option (a leading `-`), no whitespace or control character, no `:`
/// (`git_show` adds `:<path>` itself).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Revision(String);

impl Revision {
  pub fn new(value: &str) -> Result<Self, String> {
    let valid = !value.is_empty()
      && value.chars().count() <= MAX_REVISION_CHARS
      && !value.starts_with('-')
      && !value.contains(':')
      && !value.chars().any(|c| c.is_whitespace() || c.is_control());
    if valid {
      Ok(Self(value.to_owned()))
    } else {
      Err(format!("`{value}` is not a revision"))
    }
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

/// A commit message, which git reads from standard input: not blank, no NUL.
#[derive(Clone, PartialEq, Eq)]
pub struct CommitMessage(String);

impl CommitMessage {
  pub fn new(value: &str) -> Result<Self, String> {
    if value.trim().is_empty() {
      return Err("the commit message must not be blank".to_owned());
    }
    if value.contains('\0') || value.chars().count() > MAX_MESSAGE_CHARS {
      return Err("the commit message is not text git takes".to_owned());
    }
    Ok(Self(value.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl fmt::Debug for CommitMessage {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "CommitMessage({} characters)", self.0.chars().count())
  }
}

/// One call, read into its tool's input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolCall {
  /// A call of a file tool, read into the native agent's input.
  File(Box<ToolInput>),
  FindSymbol(FindSymbol),
  Git(GitCall),
  ReadOutsideFile {
    path: String,
    offset: u32,
    limit: u32,
  },
  RunProjectTool {
    name: String,
  },
  Skill(SkillCall),
}

/// A call of one of the git tools.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GitCall {
  Status,
  Diff { path: Option<RepoPath>, staged: bool },
  Log { path: Option<RepoPath>, count: u32 },
  Show { revision: Revision, path: Option<RepoPath> },
  Add { paths: Vec<RepoPath> },
  Commit { message: CommitMessage },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NoInput {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DiffInput {
  #[serde(default)]
  path: Option<String>,
  #[serde(default)]
  staged: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LogInput {
  #[serde(default)]
  path: Option<String>,
  #[serde(default)]
  count: Option<NonZeroU32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ShowInput {
  revision: String,
  #[serde(default)]
  path: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AddInput {
  paths: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommitInput {
  message: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadInput {
  path: String,
  #[serde(default)]
  offset: Option<NonZeroU32>,
  #[serde(default)]
  limit: Option<NonZeroU32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectInput {
  name: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SkillInput {
  name: String,
  #[serde(default)]
  file: Option<String>,
  #[serde(default)]
  offset: Option<NonZeroU32>,
  #[serde(default)]
  limit: Option<NonZeroU32>,
}

impl ToolCall {
  /// The call of tool `name` with `arguments`, if `step` offers that tool and the input is valid. A
  /// call without arguments is a call with none (`{}`).
  pub fn parse(name: &str, arguments: &Value, step: &StepTools) -> Result<Self, ToolCallError> {
    let tool = OrchesterTool::from_name(name).ok_or_else(|| ToolCallError::UnknownTool(name.to_owned()))?;
    if step.policy(tool).is_none() {
      return Err(ToolCallError::NotOffered(tool));
    }
    let arguments = if arguments.is_null() {
      Value::Object(serde_json::Map::new())
    } else {
      arguments.clone()
    };
    let call = read(tool, arguments).map_err(|message| ToolCallError::Invalid { tool, message })?;
    if let Self::RunProjectTool { name } = &call
      && !step.offers_project_tool(name)
    {
      return Err(ToolCallError::UnknownProjectTool(name.clone()));
    }
    Ok(call)
  }

  pub fn tool(&self) -> OrchesterTool {
    match self {
      Self::File(input) => {
        OrchesterTool::from_name(input.name().as_str()).expect("a file call is one of the file tools")
      }
      Self::FindSymbol(_) => OrchesterTool::FindSymbol,
      Self::Git(GitCall::Status) => OrchesterTool::GitStatus,
      Self::Git(GitCall::Diff { .. }) => OrchesterTool::GitDiff,
      Self::Git(GitCall::Log { .. }) => OrchesterTool::GitLog,
      Self::Git(GitCall::Show { .. }) => OrchesterTool::GitShow,
      Self::Git(GitCall::Add { .. }) => OrchesterTool::GitAdd,
      Self::Git(GitCall::Commit { .. }) => OrchesterTool::GitCommit,
      Self::ReadOutsideFile { .. } => OrchesterTool::ReadOutsideFile,
      Self::RunProjectTool { .. } => OrchesterTool::RunProjectTool,
      Self::Skill(SkillCall::List) => OrchesterTool::ListSkills,
      Self::Skill(SkillCall::Read { .. }) => OrchesterTool::ReadSkill,
    }
  }
}

/// `arguments` read as file tool `tool`'s input, as the native agent reads it.
fn file_call(tool: OrchesterTool, arguments: &Value) -> Result<ToolCall, String> {
  match parse_call(tool.as_str(), arguments) {
    Ok(input) => Ok(ToolCall::File(Box::new(input))),
    Err(ToolInputError::Invalid { message, .. }) => Err(message),
    Err(error) => Err(error.to_string()),
  }
}

/// `arguments` read as `tool`'s input, or why they are not.
fn read(tool: OrchesterTool, arguments: Value) -> Result<ToolCall, String> {
  let invalid = |error: serde_json::Error| error.to_string();
  let path = |path: Option<String>| path.as_deref().map(RepoPath::new).transpose();
  Ok(match tool {
    OrchesterTool::ReadFile
    | OrchesterTool::ListFiles
    | OrchesterTool::Search
    | OrchesterTool::EditFile
    | OrchesterTool::WriteFile => file_call(tool, &arguments)?,
    OrchesterTool::FindSymbol => {
      let query: FindSymbol = serde_json::from_value(arguments).map_err(invalid)?;
      if query.name().trim().is_empty() {
        return Err("name the item to find".to_owned());
      }
      ToolCall::FindSymbol(query)
    }
    OrchesterTool::GitStatus => {
      serde_json::from_value::<NoInput>(arguments).map_err(invalid)?;
      ToolCall::Git(GitCall::Status)
    }
    OrchesterTool::GitDiff => {
      let input: DiffInput = serde_json::from_value(arguments).map_err(invalid)?;
      ToolCall::Git(GitCall::Diff {
        path: path(input.path)?,
        staged: input.staged.unwrap_or(false),
      })
    }
    OrchesterTool::GitLog => {
      let input: LogInput = serde_json::from_value(arguments).map_err(invalid)?;
      let count = input.count.map_or(DEFAULT_LOG_COUNT, NonZeroU32::get);
      if count > MAX_LOG_COUNT {
        return Err(format!("count is at most {MAX_LOG_COUNT}"));
      }
      ToolCall::Git(GitCall::Log {
        path: path(input.path)?,
        count,
      })
    }
    OrchesterTool::GitShow => {
      let input: ShowInput = serde_json::from_value(arguments).map_err(invalid)?;
      ToolCall::Git(GitCall::Show {
        revision: Revision::new(&input.revision)?,
        path: path(input.path)?,
      })
    }
    OrchesterTool::GitAdd => add(&serde_json::from_value(arguments).map_err(invalid)?)?,
    OrchesterTool::GitCommit => {
      let input: CommitInput = serde_json::from_value(arguments).map_err(invalid)?;
      ToolCall::Git(GitCall::Commit {
        message: CommitMessage::new(&input.message)?,
      })
    }
    OrchesterTool::ReadOutsideFile => {
      let input: ReadInput = serde_json::from_value(arguments).map_err(invalid)?;
      ToolCall::ReadOutsideFile {
        path: input.path,
        offset: input.offset.map_or(1, NonZeroU32::get),
        limit: lines(input.limit)?,
      }
    }
    OrchesterTool::ListSkills => {
      serde_json::from_value::<NoInput>(arguments).map_err(invalid)?;
      ToolCall::Skill(SkillCall::List)
    }
    OrchesterTool::ReadSkill => read_skill(&serde_json::from_value(arguments).map_err(invalid)?)?,
    OrchesterTool::RunProjectTool => {
      let input: ProjectInput = serde_json::from_value(arguments).map_err(invalid)?;
      ToolCall::RunProjectTool { name: input.name }
    }
  })
}

/// The lines a read gives, [`DEFAULT_READ_LINES`] when `limit` is none.
fn lines(limit: Option<NonZeroU32>) -> Result<u32, String> {
  let limit = limit.map_or(DEFAULT_READ_LINES, NonZeroU32::get);
  if limit > MAX_READ_LINES {
    return Err(format!("limit is at most {MAX_READ_LINES}"));
  }
  Ok(limit)
}

/// A `read_skill` call: a skill name, and a reference file as a skill's file path is written, `SKILL.md` being the
/// skill's own document.
fn read_skill(input: &SkillInput) -> Result<ToolCall, String> {
  let name = input.name.trim();
  if name.is_empty() || name.chars().count() > MAX_SKILL_NAME_CHARS || name.chars().any(char::is_control) {
    return Err(format!("`{}` is not a skill name", input.name));
  }
  let file = match input.file.as_deref() {
    None | Some(SKILL_FILE) => None,
    Some(file) => Some(SkillFilePath::new(file).map_err(|error| error.to_string())?),
  };
  Ok(ToolCall::Skill(SkillCall::Read {
    name: name.to_owned(),
    file,
    offset: input.offset.map_or(1, NonZeroU32::get),
    limit: lines(input.limit)?,
  }))
}

fn add(input: &AddInput) -> Result<ToolCall, String> {
  if input.paths.is_empty() {
    return Err("name at least one path".to_owned());
  }
  if input.paths.len() > MAX_ADD_PATHS {
    return Err(format!("at most {MAX_ADD_PATHS} paths"));
  }
  let paths = input
    .paths
    .iter()
    .map(|path| RepoPath::new(path))
    .collect::<Result<_, _>>()?;
  Ok(ToolCall::Git(GitCall::Add { paths }))
}
