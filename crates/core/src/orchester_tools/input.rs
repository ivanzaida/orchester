//! The typed input of each tool, read strictly from a call's arguments (unknown fields refused), then checked into
//! values that cannot carry what the program would read as something else.

use std::num::NonZeroU32;

use serde::Deserialize;
use serde_json::Value;

use super::{FileCall, FindSymbol, OrchesterTool, SkillCall, StepTools, ToolCallError, files::term};
use crate::resource::{SKILL_FILE, SkillFilePath};

/// Lines a read returns when the call gives no `length` or `limit`.
pub(super) const DEFAULT_READ_LINES: u32 = 2000;
pub(super) const MAX_READ_LINES: u32 = 10_000;
/// The longest path a call outside the worktree may give, in characters.
const MAX_PATH_CHARS: usize = 4096;
const MAX_REVISION_CHARS: usize = 256;
/// The longest skill name a call may give, in characters.
const MAX_SKILL_NAME_CHARS: usize = 200;

/// A revision git resolves: no option (a leading `-`), no whitespace or control character, no `:`.
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

/// One call, read into its tool's input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolCall {
  /// A call of a worktree file tool.
  File(Box<FileCall>),
  FindSymbol(FindSymbol),
  /// `read_external_file`: lines of a file outside the worktree, by absolute path.
  ReadExternal {
    filename: String,
    start_line: u32,
    length: u32,
  },
  /// `search_external_file`: the files under a folder outside the worktree that hold a term.
  SearchExternal {
    base_directory: String,
    term: String,
  },
  RunProjectTool {
    name: String,
  },
  Skill(SkillCall),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NoInput {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExternalReadInput {
  filename: String,
  #[serde(default)]
  start_line: Option<NonZeroU32>,
  #[serde(default)]
  length: Option<NonZeroU32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExternalSearchInput {
  base_directory: String,
  term: String,
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
      Self::File(call) => call.tool(),
      Self::FindSymbol(_) => OrchesterTool::FindSymbol,
      Self::ReadExternal { .. } => OrchesterTool::ReadExternalFile,
      Self::SearchExternal { .. } => OrchesterTool::SearchExternalFile,
      Self::RunProjectTool { .. } => OrchesterTool::RunProjectTool,
      Self::Skill(SkillCall::List) => OrchesterTool::ListSkills,
      Self::Skill(SkillCall::Read { .. }) => OrchesterTool::ReadSkill,
    }
  }
}

/// `arguments` read as `tool`'s input, or why they are not.
fn read(tool: OrchesterTool, arguments: Value) -> Result<ToolCall, String> {
  if let Some(file) = FileCall::read(tool, &arguments) {
    return file.map(|call| ToolCall::File(Box::new(call)));
  }
  let invalid = |error: serde_json::Error| error.to_string();
  Ok(match tool {
    OrchesterTool::FindSymbol => {
      let query: FindSymbol = serde_json::from_value(arguments).map_err(invalid)?;
      if query.name().trim().is_empty() {
        return Err("name the item to find".to_owned());
      }
      ToolCall::FindSymbol(query)
    }
    OrchesterTool::ReadExternalFile => {
      let input: ExternalReadInput = serde_json::from_value(arguments).map_err(invalid)?;
      ToolCall::ReadExternal {
        filename: path(input.filename)?,
        start_line: input.start_line.map_or(1, NonZeroU32::get),
        length: lines(input.length, "length")?,
      }
    }
    OrchesterTool::SearchExternalFile => {
      let input: ExternalSearchInput = serde_json::from_value(arguments).map_err(invalid)?;
      ToolCall::SearchExternal {
        base_directory: path(input.base_directory)?,
        term: term(input.term)?,
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
    OrchesterTool::ReadFile
    | OrchesterTool::ListFiles
    | OrchesterTool::SearchFile
    | OrchesterTool::EditFile
    | OrchesterTool::CreateFile => unreachable!("the file tools are read by FileCall::read"),
  })
}

/// A path outside the worktree as a call gives it: not blank, at most [`MAX_PATH_CHARS`].
fn path(value: String) -> Result<String, String> {
  if value.trim().is_empty() || value.chars().count() > MAX_PATH_CHARS {
    return Err(format!("`{value}` is not a path"));
  }
  Ok(value)
}

/// The lines a read gives, [`DEFAULT_READ_LINES`] when `limit` is none; `field` names it in the error.
fn lines(limit: Option<NonZeroU32>, field: &str) -> Result<u32, String> {
  let limit = limit.map_or(DEFAULT_READ_LINES, NonZeroU32::get);
  if limit > MAX_READ_LINES {
    return Err(format!("{field} is at most {MAX_READ_LINES}"));
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
    limit: lines(input.limit, "limit")?,
  }))
}
