//! The native agent's tools (`docs/architecture/native-agent.md` §4): their names, the set a request
//! declares, and the typed input of each call. A call's input is read strictly into its struct (unknown
//! fields refused), so that what the schema promises and what the executors get cannot drift apart; an
//! invalid call is answered with an error result and the loop goes on.

mod input;
mod schema;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use self::{
  input::{
    AskQuestion, DEFAULT_READ_LINES, DEFAULT_TIMEOUT_SECONDS, Edit, EditFile, ListFiles, MAX_TIMEOUT_SECONDS, ReadFile,
    RunCommand, Search, WriteFile,
  },
  schema::ToolSet,
};
use crate::run::StepResult;

/// The tools a native agent can call.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolName {
  ReadFile,
  ListFiles,
  Search,
  EditFile,
  WriteFile,
  RunCommand,
  FinishStep,
  AskQuestion,
}

impl ToolName {
  pub const ALL: [Self; 8] = [
    Self::ReadFile,
    Self::ListFiles,
    Self::Search,
    Self::EditFile,
    Self::WriteFile,
    Self::RunCommand,
    Self::FinishStep,
    Self::AskQuestion,
  ];

  /// The name the model calls the tool by.
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::ReadFile => "read_file",
      Self::ListFiles => "list_files",
      Self::Search => "search",
      Self::EditFile => "edit_file",
      Self::WriteFile => "write_file",
      Self::RunCommand => "run_command",
      Self::FinishStep => "finish_step",
      Self::AskQuestion => "ask_question",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|tool| tool.as_str() == name)
  }

  /// Whether the tool only reads files: such calls of one response run together (§4.10).
  pub const fn only_reads(self) -> bool {
    matches!(self, Self::ReadFile | Self::ListFiles | Self::Search)
  }
}

/// A tool call's input, read into the tool's own type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolInput {
  ReadFile(ReadFile),
  ListFiles(ListFiles),
  Search(Search),
  EditFile(EditFile),
  WriteFile(WriteFile),
  RunCommand(RunCommand),
  FinishStep(StepResult),
  AskQuestion(AskQuestion),
}

impl ToolInput {
  pub const fn name(&self) -> ToolName {
    match self {
      Self::ReadFile(_) => ToolName::ReadFile,
      Self::ListFiles(_) => ToolName::ListFiles,
      Self::Search(_) => ToolName::Search,
      Self::EditFile(_) => ToolName::EditFile,
      Self::WriteFile(_) => ToolName::WriteFile,
      Self::RunCommand(_) => ToolName::RunCommand,
      Self::FinishStep(_) => ToolName::FinishStep,
      Self::AskQuestion(_) => ToolName::AskQuestion,
    }
  }
}

/// Why a tool call cannot be run as written. The message is the error result the model gets.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ToolInputError {
  #[error("there is no tool named `{0}`")]
  UnknownTool(String),
  #[error("`{tool}` is not available in this step", tool = .0.as_str())]
  NotOffered(ToolName),
  #[error("invalid input for `{tool}`: {message}", tool = .tool.as_str())]
  Invalid { tool: ToolName, message: String },
  /// The model wrote input that is not JSON; the result names it the way the provider asks for (§3.1).
  #[error("{}", invalid_json_input(.0))]
  NotJson(String),
}

/// The key of the input a client gives a tool call whose input was not JSON: `{"INVALID_JSON": "<raw>"}`
/// holds the text as the model wrote it, so the call can be stored and sent back.
pub const INVALID_JSON: &str = "INVALID_JSON";

/// The input of a call whose input text `raw` is not JSON.
pub fn invalid_json_input(raw: &str) -> serde_json::Value {
  serde_json::json!({ INVALID_JSON: raw })
}

/// The input of a call of tool `name`, or why it is invalid.
pub fn parse_call(name: &str, input: &serde_json::Value) -> Result<ToolInput, ToolInputError> {
  let tool = ToolName::from_name(name).ok_or_else(|| ToolInputError::UnknownTool(name.to_owned()))?;
  if let Some(raw) = not_json(input) {
    return Err(ToolInputError::NotJson(raw.to_owned()));
  }
  let invalid = |message: String| ToolInputError::Invalid { tool, message };
  let input = input.clone();
  let parsed = match tool {
    ToolName::ReadFile => serde_json::from_value(input).map(ToolInput::ReadFile),
    ToolName::ListFiles => serde_json::from_value(input).map(ToolInput::ListFiles),
    ToolName::Search => serde_json::from_value(input).map(ToolInput::Search),
    ToolName::EditFile => serde_json::from_value(input).map(ToolInput::EditFile),
    ToolName::WriteFile => serde_json::from_value(input).map(ToolInput::WriteFile),
    ToolName::RunCommand => serde_json::from_value(input).map(ToolInput::RunCommand),
    ToolName::FinishStep => serde_json::from_value(input).map(ToolInput::FinishStep),
    ToolName::AskQuestion => serde_json::from_value(input).map(ToolInput::AskQuestion),
  }
  .map_err(|error| invalid(error.to_string()))?;
  input::validate(&parsed).map_err(invalid)?;
  Ok(parsed)
}

/// The raw text of an input made by [`invalid_json_input`]; `None` for any other input.
pub fn not_json(input: &serde_json::Value) -> Option<&str> {
  let object = input.as_object()?;
  match (object.len(), object.get(INVALID_JSON)) {
    (1, Some(serde_json::Value::String(raw))) => Some(raw),
    _ => None,
  }
}
