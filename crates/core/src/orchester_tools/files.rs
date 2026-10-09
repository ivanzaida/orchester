//! The worktree file tools' input: `read_file`, `search_file`, `edit_file` and `create_file` (`list_files` keeps the
//! native agent's input), each read strictly (unknown fields refused) into a type that holds only valid values. A path
//! is checked against the step's folders by the executor, which resolves it (`crate::native::scope::FileScope`).

use std::num::NonZeroU32;

use serde::Deserialize;
use serde_json::Value;

use super::{
  OrchesterTool,
  input::{DEFAULT_READ_LINES, MAX_READ_LINES},
};
use crate::{
  connection::stream::{CallInput, EditText, Requested},
  native::tools::{ListFiles, ToolInput, ToolInputError, parse_call},
};

/// Files `search_file` lists when the call gives no `limit_files`.
pub const DEFAULT_LIMIT_FILES: u32 = 20;
pub const MAX_LIMIT_FILES: u32 = 200;
/// The longest term `search_file` and `search_external_file` take, in characters.
pub const MAX_TERM_CHARS: usize = 1000;

/// One call of a worktree file tool.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileCall {
  Read(ReadLines),
  List(ListFiles),
  Search(SearchTerm),
  Edit(LineEdit),
  Create(NewFile),
}

/// `read_file`: lines of a file, from `start_line`, at most `length` of them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadLines {
  filename: String,
  start_line: u32,
  length: u32,
}

impl ReadLines {
  pub fn filename(&self) -> &str {
    &self.filename
  }

  pub const fn start_line(&self) -> u32 {
    self.start_line
  }

  pub const fn length(&self) -> u32 {
    self.length
  }
}

/// `search_file`: the files holding `term` as literal text, at most `limit_files` of them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchTerm {
  term: String,
  limit_files: u32,
}

impl SearchTerm {
  pub fn term(&self) -> &str {
    &self.term
  }

  pub const fn limit_files(&self) -> u32 {
    self.limit_files
  }
}

/// `edit_file`: lines `start_line..=end_line` become `content`, if they are `expected` now. An `end_line` one before
/// `start_line` replaces no line: `content` goes in before `start_line`, and `expected` is empty.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineEdit {
  filename: String,
  start_line: u32,
  end_line: u32,
  expected: String,
  content: String,
}

impl LineEdit {
  pub fn filename(&self) -> &str {
    &self.filename
  }

  pub const fn start_line(&self) -> u32 {
    self.start_line
  }

  pub const fn end_line(&self) -> u32 {
    self.end_line
  }

  pub fn expected(&self) -> &str {
    &self.expected
  }

  pub fn content(&self) -> &str {
    &self.content
  }
}

/// `create_file`: a file that does not exist yet, with `content`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewFile {
  filename: String,
  content: String,
}

impl NewFile {
  pub fn filename(&self) -> &str {
    &self.filename
  }

  pub fn content(&self) -> &str {
    &self.content
  }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadInput {
  filename: String,
  #[serde(default)]
  start_line: Option<NonZeroU32>,
  #[serde(default)]
  length: Option<NonZeroU32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchInput {
  term: String,
  #[serde(default)]
  limit_files: Option<NonZeroU32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EditInput {
  filename: String,
  start_line: NonZeroU32,
  end_line: u32,
  expected: String,
  content: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateInput {
  filename: String,
  content: String,
}

impl FileCall {
  /// `arguments` read as worktree file tool `tool`'s input, or why they are not; `None` for another tool.
  pub fn read(tool: OrchesterTool, arguments: &Value) -> Option<Result<Self, String>> {
    let invalid = |error: serde_json::Error| error.to_string();
    Some(match tool {
      OrchesterTool::ReadFile => ReadInput::deserialize(arguments).map_err(invalid).and_then(read_lines),
      OrchesterTool::ListFiles => list(arguments),
      OrchesterTool::SearchFile => SearchInput::deserialize(arguments)
        .map_err(invalid)
        .and_then(search_term),
      OrchesterTool::EditFile => EditInput::deserialize(arguments).map_err(invalid).and_then(line_edit),
      OrchesterTool::CreateFile => CreateInput::deserialize(arguments).map_err(invalid).and_then(new_file),
      _ => return None,
    })
  }

  pub const fn tool(&self) -> OrchesterTool {
    match self {
      Self::Read(_) => OrchesterTool::ReadFile,
      Self::List(_) => OrchesterTool::ListFiles,
      Self::Search(_) => OrchesterTool::SearchFile,
      Self::Edit(_) => OrchesterTool::EditFile,
      Self::Create(_) => OrchesterTool::CreateFile,
    }
  }

  /// What the call asked, as the native agent's row of the same kind shows it (04.H10).
  pub fn call_input(&self) -> Option<CallInput> {
    Some(match self {
      Self::Read(read) => CallInput::requested(read.filename(), None, None),
      Self::List(list) => CallInput::of(&ToolInput::ListFiles(list.clone()))?,
      Self::Search(search) => CallInput::requested(search.term(), None, None),
      Self::Edit(edit) => {
        let edits = vec![EditText::new(edit.expected(), edit.content(), false)];
        CallInput::requested(edit.filename(), None, Some(Requested::Edits(edits)))
      }
      Self::Create(create) => CallInput::requested(
        create.filename(),
        None,
        Some(Requested::Content(create.content().to_owned())),
      ),
    })
  }
}

/// `list_files` reads the native agent's input.
fn list(arguments: &Value) -> Result<FileCall, String> {
  match parse_call(OrchesterTool::ListFiles.as_str(), arguments) {
    Ok(ToolInput::ListFiles(input)) => Ok(FileCall::List(input)),
    Ok(_) => Err("this is no list_files input".to_owned()),
    Err(ToolInputError::Invalid { message, .. }) => Err(message),
    Err(error) => Err(error.to_string()),
  }
}

fn read_lines(input: ReadInput) -> Result<FileCall, String> {
  let length = input.length.map_or(DEFAULT_READ_LINES, NonZeroU32::get);
  if length > MAX_READ_LINES {
    return Err(format!("length is at most {MAX_READ_LINES}"));
  }
  Ok(FileCall::Read(ReadLines {
    filename: filename(input.filename)?,
    start_line: input.start_line.map_or(1, NonZeroU32::get),
    length,
  }))
}

fn search_term(input: SearchInput) -> Result<FileCall, String> {
  let limit_files = input.limit_files.map_or(DEFAULT_LIMIT_FILES, NonZeroU32::get);
  if limit_files > MAX_LIMIT_FILES {
    return Err(format!("limit_files is at most {MAX_LIMIT_FILES}"));
  }
  Ok(FileCall::Search(SearchTerm {
    term: term(input.term)?,
    limit_files,
  }))
}

fn line_edit(input: EditInput) -> Result<FileCall, String> {
  let start_line = input.start_line.get();
  if input.end_line < start_line - 1 {
    return Err(format!(
      "end_line {} is before start_line {start_line}; use start_line - 1 to insert without replacing",
      input.end_line
    ));
  }
  if input.end_line == start_line - 1 && !input.expected.is_empty() {
    return Err("an insertion replaces no line: expected must be empty".to_owned());
  }
  if input.expected == input.content {
    return Err("the edit changes nothing: expected and content are the same".to_owned());
  }
  Ok(FileCall::Edit(LineEdit {
    filename: filename(input.filename)?,
    start_line,
    end_line: input.end_line,
    expected: input.expected,
    content: input.content,
  }))
}

fn new_file(input: CreateInput) -> Result<FileCall, String> {
  Ok(FileCall::Create(NewFile {
    filename: filename(input.filename)?,
    content: input.content,
  }))
}

/// A file name that is not blank.
fn filename(value: String) -> Result<String, String> {
  if value.trim().is_empty() {
    return Err("filename must not be blank".to_owned());
  }
  Ok(value)
}

/// A search term: one line of text, not blank, at most [`MAX_TERM_CHARS`].
pub(super) fn term(value: String) -> Result<String, String> {
  if value.trim().is_empty() {
    return Err("term must not be blank".to_owned());
  }
  if value.chars().count() > MAX_TERM_CHARS || value.contains(['\n', '\r', '\0']) {
    return Err(format!("term is one line of at most {MAX_TERM_CHARS} characters"));
  }
  Ok(value)
}
