//! What an agent is told about each tool: its description and input schema. Schemas are strict: no
//! property besides the listed ones, each typed and bounded; optional ones may be left out or `null`.
//! `run_project_tool`'s schema names the project tools the step is offered, so it is built per step.

use serde_json::{Value, json};

use super::{
  OrchesterTool,
  input::{DEFAULT_LOG_COUNT, DEFAULT_READ_LINES, MAX_ADD_PATHS, MAX_LOG_COUNT, MAX_READ_LINES},
};
use crate::native::tools::ToolName as NativeTool;

const FIND_SYMBOL: &str = r#"{"type":"object","properties":{
  "name":{"type":"string","description":"The item's name, or part of it; any case."},
  "kind":{"type":["string","null"],"description":"Only items of this kind: struct, enum, trait, function, method, class, interface, type, module, macro, const, static, impl."},
  "include_tests":{"type":["boolean","null"],"description":"Also definitions in test folders; null for false."},
  "include_consts":{"type":["boolean","null"],"description":"Also constants and statics; null for false."}
},"required":["name"],"additionalProperties":false}"#;

const NO_INPUT: &str = r#"{"type":"object","properties":{},"additionalProperties":false}"#;

const GIT_DIFF: &str = r#"{"type":"object","properties":{
  "path":{"type":["string","null"],"description":"File or folder, relative to the worktree root; null for every change."},
  "staged":{"type":["boolean","null"],"description":"Compare the staged changes with HEAD instead of the working files; null for false."}
},"additionalProperties":false}"#;

const GIT_SHOW: &str = r#"{"type":"object","properties":{
  "revision":{"type":"string","description":"A commit, branch or tag, such as HEAD~1."},
  "path":{"type":["string","null"],"description":"A file relative to the worktree root: show its content at the revision; null shows the commit."}
},"required":["revision"],"additionalProperties":false}"#;

const GIT_COMMIT: &str = r#"{"type":"object","properties":{
  "message":{"type":"string","description":"The commit message: a subject line, optionally a blank line and a body."}
},"required":["message"],"additionalProperties":false}"#;

impl OrchesterTool {
  /// What the agent is told the tool does.
  pub const fn description(self) -> &'static str {
    match self {
      Self::ReadFile => NativeTool::ReadFile.description(),
      Self::ListFiles => NativeTool::ListFiles.description(),
      Self::Search => NativeTool::Search.description(),
      Self::EditFile => NativeTool::EditFile.description(),
      Self::WriteFile => NativeTool::WriteFile.description(),
      Self::FindSymbol => {
        "Find where a type, function, method, constant or other item is defined, from the code index: path:line,          kind and signature, best matches first. Faster than searching file contents for a definition."
      }
      Self::GitStatus => "Show the worktree's branch and changed files (git status --short --branch).",
      Self::GitDiff => "Show uncommitted changes (git diff), or the staged ones, optionally for one path.",
      Self::GitLog => "List recent commits (git log), optionally those that changed one path.",
      Self::GitShow => "Show a commit (git show), or a file's content at a revision.",
      Self::GitAdd => "Stage the named files for the next commit (git add); folders are refused, name each file.",
      Self::GitCommit => "Commit the staged changes with a message (git commit). Hooks do not run.",
      Self::ReadOutsideFile => {
        "Read a text file outside the worktree, such as a dependency's source, by absolute path. Credential \
         locations are refused. A person may be asked first."
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
    matches!(
      self,
      Self::ReadFile
        | Self::ListFiles
        | Self::Search
        | Self::FindSymbol
        | Self::GitStatus
        | Self::GitDiff
        | Self::GitLog
        | Self::GitShow
        | Self::ReadOutsideFile
        | Self::ListSkills
        | Self::ReadSkill
    )
  }
}

/// The input schema of `tool`; `projects` are the names and descriptions of the project tools
/// `run_project_tool` may run.
pub(super) fn input_schema(tool: OrchesterTool, projects: &[(String, String)]) -> Value {
  let fixed = |text: &str| serde_json::from_str::<Value>(text).expect("the static tool schemas are valid JSON");
  match tool {
    OrchesterTool::ReadFile => fixed(NativeTool::ReadFile.input_schema()),
    OrchesterTool::ListFiles => fixed(NativeTool::ListFiles.input_schema()),
    OrchesterTool::Search => fixed(NativeTool::Search.input_schema()),
    OrchesterTool::EditFile => fixed(NativeTool::EditFile.input_schema()),
    OrchesterTool::WriteFile => fixed(NativeTool::WriteFile.input_schema()),
    OrchesterTool::FindSymbol => fixed(FIND_SYMBOL),
    OrchesterTool::GitStatus | OrchesterTool::ListSkills => fixed(NO_INPUT),
    OrchesterTool::GitDiff => fixed(GIT_DIFF),
    OrchesterTool::GitShow => fixed(GIT_SHOW),
    OrchesterTool::GitCommit => fixed(GIT_COMMIT),
    OrchesterTool::GitLog => json!({"type":"object","properties":{
      "path":{"type":["string","null"],"description":"Only commits that changed this path, relative to the worktree root."},
      "count":{"type":["integer","null"],"minimum":1,"maximum":MAX_LOG_COUNT,
        "description":format!("How many commits; null for {DEFAULT_LOG_COUNT}.")}
    },"additionalProperties":false}),
    OrchesterTool::GitAdd => json!({"type":"object","properties":{
      "paths":{"type":"array","minItems":1,"maxItems":MAX_ADD_PATHS,"items":{"type":"string"},
        "description":"Files, relative to the worktree root; no folders."}
    },"required":["paths"],"additionalProperties":false}),
    OrchesterTool::ReadOutsideFile => json!({"type":"object","properties":{
      "path":{"type":"string","description":"Absolute path of the file."},
      "offset":{"type":["integer","null"],"minimum":1,"description":"1-based line to start at; null for line 1."},
      "limit":{"type":["integer","null"],"minimum":1,"maximum":MAX_READ_LINES,
        "description":format!("Number of lines; null for {DEFAULT_READ_LINES}.")}
    },"required":["path"],"additionalProperties":false}),
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
