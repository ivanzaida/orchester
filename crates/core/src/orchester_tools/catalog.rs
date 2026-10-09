//! What an agent is told about each tool: its description and input schema. Schemas are strict: no
//! property besides the listed ones, each typed and bounded; optional ones may be left out or `null`.
//! `run_project_tool`'s schema names the project tools the step is offered, so it is built per step.

use serde_json::{Value, json};

use super::{
  OrchesterTool,
  files::{DEFAULT_LIMIT_FILES, MAX_LIMIT_FILES, MAX_TERM_CHARS},
  input::{DEFAULT_READ_LINES, MAX_READ_LINES},
};
use crate::native::tools::ToolName as NativeTool;

const FIND_SYMBOL: &str = r#"{"type":"object","properties":{
  "name":{"type":"string","description":"The item's name, or part of it; any case."},
  "kind":{"type":["string","null"],"description":"Only items of this kind: struct, enum, trait, function, method, class, interface, type, module, macro, const, static, impl."},
  "include_tests":{"type":["boolean","null"],"description":"Also definitions in test folders; null for false."},
  "include_consts":{"type":["boolean","null"],"description":"Also constants and statics; null for false."}
},"required":["name"],"additionalProperties":false}"#;

const NO_INPUT: &str = r#"{"type":"object","properties":{},"additionalProperties":false}"#;

const EDIT_FILE: &str = r#"{"type":"object","properties":{
  "filename":{"type":"string","description":"File path, relative to the worktree root."},
  "start_line":{"type":"integer","minimum":1,"description":"The first line to replace, 1-based, as read_file numbers it."},
  "end_line":{"type":"integer","minimum":0,"description":"The last line to replace. start_line - 1 replaces no line: content goes in before start_line."},
  "expected":{"type":"string","description":"Lines start_line to end_line exactly as they are now, without their numbers; empty for an insertion."},
  "content":{"type":"string","description":"The lines that replace them; empty removes them."}
},"required":["filename","start_line","end_line","expected","content"],"additionalProperties":false}"#;

const CREATE_FILE: &str = r#"{"type":"object","properties":{
  "filename":{"type":"string","description":"Path of the new file, relative to the worktree root; missing folders are made."},
  "content":{"type":"string","description":"The file's whole content."}
},"required":["filename","content"],"additionalProperties":false}"#;

impl OrchesterTool {
  /// What the agent is told the tool does.
  pub const fn description(self) -> &'static str {
    match self {
      Self::ReadFile => {
        "Read a text file of the worktree as numbered lines. Read a file before you edit it: edit_file needs this \
         read."
      }
      Self::ListFiles => NativeTool::ListFiles.description(),
      Self::SearchFile => {
        "Search the worktree's files for literal text: for a name, its definitions from the code index first; then \
         each matching file with its matching lines, at most limit_files files."
      }
      Self::EditFile => {
        "Replace lines start_line to end_line of a file you have read with content, only if they are exactly \
         expected now; otherwise nothing changes and you get what they hold now. Line numbers after the range \
         shift by the lines added or removed."
      }
      Self::CreateFile => {
        "Create a new file with the given content. It is refused when the file exists: use edit_file."
      }
      Self::FindSymbol => {
        "Find where a type, function, method, constant or other item is defined, from the code index: path:line, \
         kind and signature, best matches first. Faster than searching file contents for a definition."
      }
      Self::SearchExternalFile => {
        "Search the files under a folder outside the worktree, such as a dependency's sources, for literal text. \
         A person may be asked first. Credential locations are refused."
      }
      Self::ReadExternalFile => {
        "Read a text file outside the worktree, such as a dependency's source, by absolute path. A person may be \
         asked first. Credential locations are refused."
      }
      Self::RunProjectTool => {
        "Run one of the project's command tools by name, with its fixed arguments, in the worktree. A person may \
         be asked first."
      }
      Self::ListSkills => {
        "List the skills this step has, Orchester's and the repository's, each with its source, description and \
         reference files."
      }
      Self::ReadSkill => {
        "Read a skill's SKILL.md, or one of its reference files, by the skill's name. Use this instead of a path to \
         read a skill."
      }
    }
  }

  /// Whether the tool changes nothing, as MCP annotates it (`readOnlyHint`).
  pub const fn read_only(self) -> bool {
    !matches!(self, Self::EditFile | Self::CreateFile | Self::RunProjectTool)
  }
}

/// The input schema of `tool`; `projects` are the names and descriptions of the project tools
/// `run_project_tool` may run.
pub(super) fn input_schema(tool: OrchesterTool, projects: &[(String, String)]) -> Value {
  let fixed = |text: &str| serde_json::from_str::<Value>(text).expect("the static tool schemas are valid JSON");
  let read_lines = |path: &str| {
    json!({"type":"object","properties":{
      "filename":{"type":"string","description":path},
      "start_line":{"type":["integer","null"],"minimum":1,"description":"1-based line to start at; null for line 1."},
      "length":{"type":["integer","null"],"minimum":1,"maximum":MAX_READ_LINES,
        "description":format!("Number of lines; null for {DEFAULT_READ_LINES}.")}
    },"required":["filename"],"additionalProperties":false})
  };
  let term = json!({"type":"string","maxLength":MAX_TERM_CHARS,
    "description":"Literal text on one line, not a pattern; matched ignoring case when it is all lower case."});
  match tool {
    OrchesterTool::ReadFile => read_lines("File path, relative to the worktree root."),
    OrchesterTool::ListFiles => fixed(NativeTool::ListFiles.input_schema()),
    OrchesterTool::SearchFile => json!({"type":"object","properties":{
      "term":term,
      "limit_files":{"type":["integer","null"],"minimum":1,"maximum":MAX_LIMIT_FILES,
        "description":format!("The most files to list; null for {DEFAULT_LIMIT_FILES}.")}
    },"required":["term"],"additionalProperties":false}),
    OrchesterTool::EditFile => fixed(EDIT_FILE),
    OrchesterTool::CreateFile => fixed(CREATE_FILE),
    OrchesterTool::FindSymbol => fixed(FIND_SYMBOL),
    OrchesterTool::SearchExternalFile => json!({"type":"object","properties":{
      "base_directory":{"type":"string","description":"Absolute path of the folder to search in."},
      "term":term
    },"required":["base_directory","term"],"additionalProperties":false}),
    OrchesterTool::ReadExternalFile => read_lines("Absolute path of the file."),
    OrchesterTool::ListSkills => fixed(NO_INPUT),
    OrchesterTool::ReadSkill => json!({"type":"object","properties":{
      "name":{"type":"string","description":"The skill's name, as list_skills or the prompt names it."},
      "file":{"type":["string","null"],"description":"A reference file, relative to the skill's folder with / \
        between names, as list_skills lists it; null for the skill's SKILL.md."},
      "offset":{"type":["integer","null"],"minimum":1,"description":"1-based line to start at; null for line 1."},
      "limit":{"type":["integer","null"],"minimum":1,"maximum":MAX_READ_LINES,
        "description":format!("Number of lines; null for {DEFAULT_READ_LINES}.")}
    },"required":["name"],"additionalProperties":false}),
    OrchesterTool::RunProjectTool => {
      let names: Vec<&str> = projects.iter().map(|(name, _)| name.as_str()).collect();
      json!({"type":"object","properties":{
        "name":{"type":"string","enum":names,"description":project_list(projects)}
      },"required":["name"],"additionalProperties":false})
    }
  }
}

/// The project tools, one per line with what each does, for the description of `name`.
fn project_list(projects: &[(String, String)]) -> String {
  let mut text = "The project tool to run:".to_owned();
  for (name, description) in projects {
    text.push_str("\n- ");
    text.push_str(name);
    if !description.is_empty() {
      text.push_str(": ");
      text.push_str(description);
    }
  }
  text
}
