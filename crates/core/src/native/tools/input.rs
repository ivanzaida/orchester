//! The typed inputs of the native tools (`docs/architecture/native-agent.md` §4.1–§4.8). Each matches its
//! schema in [`super::schema`]: every property, optional ones nullable, nothing else.

use std::num::NonZeroU32;

use serde::Deserialize;

use super::ToolInput;
use crate::question::{Question, QuestionError, QuestionOption};

/// Lines `read_file` returns when the call gives no `limit` (§4.1).
pub const DEFAULT_READ_LINES: u32 = 2000;
/// Seconds a command may run when the call gives no `timeout_seconds` (§4.6).
pub const DEFAULT_TIMEOUT_SECONDS: u32 = 120;
/// The longest `timeout_seconds` a call may ask for.
pub const MAX_TIMEOUT_SECONDS: u32 = 1800;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadFile {
  path: String,
  offset: Option<NonZeroU32>,
  limit: Option<NonZeroU32>,
}

impl ReadFile {
  pub fn path(&self) -> &str {
    &self.path
  }

  /// The 1-based line to start at.
  pub fn offset(&self) -> u32 {
    self.offset.map_or(1, NonZeroU32::get)
  }

  /// How many lines to read.
  pub fn limit(&self) -> u32 {
    self.limit.map_or(DEFAULT_READ_LINES, NonZeroU32::get)
  }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListFiles {
  pattern: Option<String>,
  path: Option<String>,
}

impl ListFiles {
  /// A glob relative to [`Self::path`].
  pub fn pattern(&self) -> Option<&str> {
    self.pattern.as_deref()
  }

  /// The folder to list; the worktree root when `None`.
  pub fn path(&self) -> Option<&str> {
    self.path.as_deref()
  }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Search {
  pattern: String,
  path: Option<String>,
  glob: Option<String>,
  case_insensitive: Option<bool>,
  context: Option<u32>,
}

impl Search {
  /// A regular expression, as ripgrep reads it.
  pub fn pattern(&self) -> &str {
    &self.pattern
  }

  pub fn path(&self) -> Option<&str> {
    self.path.as_deref()
  }

  pub fn glob(&self) -> Option<&str> {
    self.glob.as_deref()
  }

  pub fn case_insensitive(&self) -> bool {
    self.case_insensitive.unwrap_or(false)
  }

  /// Lines of context around each match.
  pub fn context(&self) -> u32 {
    self.context.unwrap_or(0)
  }
}

/// One replacement of an `edit_file` call.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edit {
  old: String,
  new: String,
  replace_all: Option<bool>,
}

impl Edit {
  pub fn old(&self) -> &str {
    &self.old
  }

  pub fn new_text(&self) -> &str {
    &self.new
  }

  /// Replace every occurrence; otherwise `old` must occur exactly once.
  pub fn replace_all(&self) -> bool {
    self.replace_all.unwrap_or(false)
  }
}

/// One file edit: several edits apply in order, and all or none of them (§4.4).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditFile {
  path: String,
  edits: Vec<Edit>,
}

impl EditFile {
  pub fn path(&self) -> &str {
    &self.path
  }

  pub fn edits(&self) -> &[Edit] {
    &self.edits
  }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WriteFile {
  path: String,
  content: String,
}

impl WriteFile {
  pub fn path(&self) -> &str {
    &self.path
  }

  pub fn content(&self) -> &str {
    &self.content
  }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunCommand {
  command: String,
  shell: Option<bool>,
  workdir: Option<String>,
  timeout_seconds: Option<NonZeroU32>,
}

impl RunCommand {
  pub fn command(&self) -> &str {
    &self.command
  }

  /// Run the string in the platform shell instead of starting the program directly (§4.6).
  pub fn shell(&self) -> bool {
    self.shell.unwrap_or(false)
  }

  /// The working folder; the worktree root when `None`.
  pub fn workdir(&self) -> Option<&str> {
    self.workdir.as_deref()
  }

  pub fn timeout_seconds(&self) -> u32 {
    self.timeout_seconds.map_or(DEFAULT_TIMEOUT_SECONDS, NonZeroU32::get)
  }
}

/// One answer an `ask_question` call suggests.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AskOption {
  label: String,
  description: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AskQuestion {
  question: String,
  header: Option<String>,
  options: Option<Vec<AskOption>>,
  multi_select: Option<bool>,
}

impl AskQuestion {
  pub fn question(&self) -> &str {
    &self.question
  }

  /// The question with the answers it suggests, which the person picks from or answers in their own
  /// words; `None` for a plain question, which suggests none.
  pub fn structured(&self) -> Option<Question> {
    self
      .structure()
      .map(|question| question.expect("a call's question is checked when the call is read"))
  }

  fn structure(&self) -> Option<Result<Question, QuestionError>> {
    let options = self.options.as_deref().filter(|options| !options.is_empty())?;
    let options = options
      .iter()
      .map(|option| QuestionOption::new(&option.label, option.description.as_deref()))
      .collect();
    Some(Question::new(
      &self.question,
      self.header.as_deref(),
      options,
      self.multi_select.unwrap_or(false),
    ))
  }
}

/// What serde cannot check: blank text, empty lists, edits that change nothing, too long timeouts.
pub(super) fn validate(input: &ToolInput) -> Result<(), String> {
  match input {
    ToolInput::ReadFile(read) => filled("path", &read.path),
    ToolInput::ListFiles(_) | ToolInput::FinishStep(_) => Ok(()),
    ToolInput::Search(search) => filled("pattern", &search.pattern),
    ToolInput::EditFile(edit) => validate_edit(edit),
    ToolInput::WriteFile(write) => filled("path", &write.path),
    ToolInput::RunCommand(command) => validate_command(command),
    ToolInput::AskQuestion(ask) => {
      filled("question", &ask.question)?;
      match ask.structure() {
        Some(Err(error)) => Err(error.to_string()),
        Some(Ok(_)) | None => Ok(()),
      }
    }
  }
}

fn validate_edit(edit: &EditFile) -> Result<(), String> {
  filled("path", &edit.path)?;
  if edit.edits.is_empty() {
    return Err("`edits` must hold at least one edit".to_owned());
  }
  for (index, one) in edit.edits.iter().enumerate() {
    if one.old.is_empty() {
      return Err(format!("edit {} has an empty `old`", index + 1));
    }
    if one.old == one.new {
      return Err(format!(
        "edit {} changes nothing: `old` and `new` are the same",
        index + 1
      ));
    }
  }
  Ok(())
}

fn validate_command(command: &RunCommand) -> Result<(), String> {
  filled("command", &command.command)?;
  if command.timeout_seconds() > MAX_TIMEOUT_SECONDS {
    return Err(format!("`timeout_seconds` is at most {MAX_TIMEOUT_SECONDS}"));
  }
  Ok(())
}

fn filled(field: &str, value: &str) -> Result<(), String> {
  if value.trim().is_empty() {
    Err(format!("`{field}` must not be blank"))
  } else {
    Ok(())
  }
}
