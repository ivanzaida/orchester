//! What a request declares about each tool: its description and JSON schema. The schemas are
//! strict-compatible (§4): `additionalProperties: false`, every property required, optional ones nullable.

use super::ToolName;
use crate::{resource::RepositoryAccess, run::STEP_RESULT_SCHEMA};

const READ_FILE: &str = r#"{"type":"object","properties":{
  "path":{"type":"string","description":"File path, relative to the worktree root."},
  "offset":{"type":["integer","null"],"minimum":1,"description":"1-based line to start at; null for line 1."},
  "limit":{"type":["integer","null"],"minimum":1,"description":"Number of lines to read; null for 2000."}
},"required":["path","offset","limit"],"additionalProperties":false}"#;

const LIST_FILES: &str = r#"{"type":"object","properties":{
  "pattern":{"type":["string","null"],"description":"Glob relative to path, such as **/*.rs; null lists every file."},
  "path":{"type":["string","null"],"description":"Folder to list, relative to the worktree root; null for the root."}
},"required":["pattern","path"],"additionalProperties":false}"#;

const SEARCH: &str = r#"{"type":"object","properties":{
  "pattern":{"type":"string","description":"Regular expression, in ripgrep syntax."},
  "path":{"type":["string","null"],"description":"File or folder to search; null for the worktree root."},
  "glob":{"type":["string","null"],"description":"Only search files matching this glob."},
  "case_insensitive":{"type":["boolean","null"],"description":"Ignore case; null for false."},
  "context":{"type":["integer","null"],"minimum":0,"description":"Lines of context around each match; null for 0."}
},"required":["pattern","path","glob","case_insensitive","context"],"additionalProperties":false}"#;

const EDIT_FILE: &str = r#"{"type":"object","properties":{
  "path":{"type":"string","description":"File path, relative to the worktree root."},
  "edits":{"type":"array","minItems":1,"description":"Replacements applied in order; all or none are written.","items":{
    "type":"object","properties":{
      "old":{"type":"string","description":"Exact text to replace; it must occur once unless replace_all."},
      "new":{"type":"string","description":"Replacement text."},
      "replace_all":{"type":["boolean","null"],"description":"Replace every occurrence; null for false."}
    },"required":["old","new","replace_all"],"additionalProperties":false}}
},"required":["path","edits"],"additionalProperties":false}"#;

const WRITE_FILE: &str = r#"{"type":"object","properties":{
  "path":{"type":"string","description":"File path, relative to the worktree root."},
  "content":{"type":"string","description":"The whole new content of the file."}
},"required":["path","content"],"additionalProperties":false}"#;

const RUN_COMMAND: &str = r#"{"type":"object","properties":{
  "command":{"type":"string","description":"Program and arguments. Without shell, no shell operators."},
  "shell":{"type":["boolean","null"],"description":"Run the string in the platform shell; such commands always ask a person."},
  "workdir":{"type":["string","null"],"description":"Working folder inside the worktree; null for the root."},
  "timeout_seconds":{"type":["integer","null"],"minimum":1,"maximum":1800,"description":"Time limit; null for 120."}
},"required":["command","shell","workdir","timeout_seconds"],"additionalProperties":false}"#;

const ASK_QUESTION: &str = r#"{"type":"object","properties":{
  "question":{"type":"string","description":"The question, with what you need to continue."},
  "header":{"type":["string","null"],"description":"A short label for what the question is about, a few words; null for none."},
  "options":{"type":["array","null"],"maxItems":26,"description":"Suggested answers, at least two with different labels; null for a plain question. The person may also answer in their own words.","items":{
    "type":"object","properties":{
      "label":{"type":"string","description":"The answer, short; it comes back as the answer when picked."},
      "description":{"type":["string","null"],"description":"What picking it means; null for none."}
    },"required":["label","description"],"additionalProperties":false}},
  "multi_select":{"type":["boolean","null"],"description":"The person may pick several options; null for one."}
},"required":["question","header","options","multi_select"],"additionalProperties":false}"#;

impl ToolName {
  /// The tool's input schema, as JSON.
  pub const fn input_schema(self) -> &'static str {
    match self {
      Self::ReadFile => READ_FILE,
      Self::ListFiles => LIST_FILES,
      Self::Search => SEARCH,
      Self::EditFile => EDIT_FILE,
      Self::WriteFile => WRITE_FILE,
      Self::RunCommand => RUN_COMMAND,
      Self::FinishStep => STEP_RESULT_SCHEMA,
      Self::AskQuestion => ASK_QUESTION,
    }
  }

  /// What the model is told the tool does.
  pub const fn description(self) -> &'static str {
    match self {
      Self::ReadFile => "Read a text file as numbered lines. Read a file before you edit or replace it.",
      Self::ListFiles => "List files, respecting .gitignore. Sorted, at most 1000 entries.",
      Self::Search => "Search file contents with ripgrep. At most 200 matches.",
      Self::EditFile => {
        "Replace exact text in a file you have read. Several edits apply in order, and all or none are written."
      }
      Self::WriteFile => "Create a file, or replace one you have read, with the given content.",
      Self::RunCommand => {
        "Run a command in the worktree. Listed and known-safe read-only commands run; others ask a person, who may \
         deny them."
      }
      Self::FinishStep => "Report the step's result. Call it once, when you are done; it ends your work on the step.",
      Self::AskQuestion => {
        "Ask the person a question you need answered to continue. Your work waits for the answer, which comes back \
         as this tool's result."
      }
    }
  }
}

/// The tools one conversation declares. A step that only reads gets no `edit_file` or `write_file`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolSet(Vec<ToolName>);

impl ToolSet {
  pub fn for_access(access: RepositoryAccess) -> Self {
    let tools = ToolName::ALL
      .into_iter()
      .filter(|tool| access == RepositoryAccess::Write || !matches!(tool, ToolName::EditFile | ToolName::WriteFile));
    Self(tools.collect())
  }

  /// No tools: a request that only asks for an answer (auto mode's review).
  pub const fn none() -> Self {
    Self(Vec::new())
  }

  pub fn tools(&self) -> &[ToolName] {
    &self.0
  }

  pub fn offers(&self, tool: ToolName) -> bool {
    self.0.contains(&tool)
  }
}
