//! What an agent is told about each tool: its description and input schema. Schemas are strict: no
//! property besides the listed ones, each typed and bounded; optional ones may be left out or `null`.
//! `run_check`'s schema names the checks the step is offered, so it is built per step.

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

const DELETE_FILE: &str = r#"{"type":"object","properties":{
  "filename":{"type":"string","description":"The file to delete, relative to the worktree root."}
},"required":["filename"],"additionalProperties":false}"#;

const MOVE_FILE: &str = r#"{"type":"object","properties":{
  "from":{"type":"string","description":"The file to move, relative to the worktree root."},
  "to":{"type":"string","description":"Where it goes, relative to the worktree root; no file may be there. Missing folders are made."}
},"required":["from","to"],"additionalProperties":false}"#;

const FIND_REFERENCES: &str = r#"{"type":"object","properties":{
  "name":{"type":"string","description":"The name of a type, function, method or other item, exactly as code writes it."}
},"required":["name"],"additionalProperties":false}"#;

const SAVE_MEMORY: &str = r#"{"type":"object","properties":{
  "kind":{"type":"string","enum":["fact","decision","observation","artifact","handoff","todo","warning","summary"],"description":"What the record holds."},
  "text":{"type":"string","description":"What to remember, in a few sentences. Never a secret."},
  "key":{"type":["string","null"],"description":"A short name; a record with the same key in the same memory is replaced. Null for none."},
  "tags":{"type":["array","null"],"items":{"type":"string"},"description":"Words to find it by, one word each; null for none."},
  "importance":{"type":["string","null"],"enum":["low","normal","high",null],"description":"Null for normal."},
  "scope":{"type":["string","null"],"enum":["task","project",null],"description":"The task's memory, or the project's for every task; null for the task's."}
},"required":["kind","text"],"additionalProperties":false}"#;

const REPORT_PROGRESS: &str = r#"{"type":"object","properties":{
  "text":{"type":"string","maxLength":200,"description":"One short line on what the step is doing now."}
},"required":["text"],"additionalProperties":false}"#;

const START_QA_APP: &str = r#"{"type":"object","properties":{
  "scenario":{"type":"string","pattern":"^[a-z0-9][a-z0-9-]*$","maxLength":64,"description":"The seed scenario the copy starts on, as tools/qa-app.ps1 lists them (tasks, task-states, decisions, ...)."},
  "fake_agents":{"type":["boolean","null"],"description":"Run the copy's agents as Orchester's stand-ins; null for false."},
  "no_runner":{"type":["boolean","null"],"description":"Start the copy without a runner; null for false."}
},"required":["scenario"],"additionalProperties":false}"#;

const CREATE_FILE: &str = r#"{"type":"object","properties":{
  "filename":{"type":"string","description":"Path of the new file, relative to the worktree root; missing folders are made."},
  "content":{"type":"string","description":"The file's whole content."}
},"required":["filename","content"],"additionalProperties":false}"#;

impl OrchesterTool {
  /// What the agent is told the tool does.
  pub const fn description(self) -> &'static str {
    match self {
      Self::ListRunProcesses => {
        "List processes whose resolved executable lies inside this run's worktree: pid, name, relative executable path and start time."
      }
      Self::StopRunProcess => {
        "Stop a process and its children inside this run's worktree without asking: request close, wait briefly, then end survivors. Outside executables, the runner and agent CLIs are refused. Reports closed, ended or already gone."
      }
      Self::StartQaApp => {
        "Build and start a QA copy of the app from this worktree on a seeded scenario (tools/qa-app.ps1), off-screen and without focus, and make it the app call_run_app calls when you leave out its pid. As many QA apps run on this computer at once as the project's setting QA apps at once allows: while other runs' QA apps hold every slot, the call waits until one is free. Starting again replaces this step's QA app; Orchester stops it when your turn ends or waits for a person. A person may be asked first. Never start tools/qa-app.ps1 yourself."
      }
      Self::CallRunApp => {
        "Call one embedded MCP tool of an app started inside this run's worktree, without asking. Leave out pid to call the QA app start_qa_app started in this step. Other executables and mismatched discovery files are refused. The relay keeps the token private; PNG images are saved under .tmp/ and returned as relative paths: open one with read_file to see it. Use this instead of tools/lurq-mcp-client.py."
      }
      Self::ReadFile => {
        "Read a text file of the worktree as numbered lines. Read a file before you edit it: edit_file needs this \
         read. On Claude Code connections a PNG, JPEG or WebP file (told by its content) is shown as the image, at \
         most 1568 pixels on its long side; start_line and length do not apply to it. Elsewhere an image is only \
         named with its size."
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
      Self::DeleteFile => "Delete a file of the worktree. The deletion shows in the run's changes.",
      Self::MoveFile => {
        "Move or rename a file of the worktree to a path where no file is. The move shows in the run's changes."
      }
      Self::FindReferences => {
        "Find where a name is used in the worktree: its definitions from the code index first, then every file \
         that uses it as a whole word, with the lines. Says when the code index is off or out of date."
      }
      Self::GetTask => {
        "The run's task: its title, brief and acceptance criteria, the plans approved in this run, and the \
         person's decisions and notes (requested changes, answers to questions, messages)."
      }
      Self::SaveMemory => {
        "Keep a record in the task's memory, or the project's, for later steps and runs. It is active at once or \
         waits for a person's approval, as the project's memory setting says."
      }
      Self::ReportProgress => {
        "Say in one short line what the step is doing now; the run's summary shows it. It replaces your earlier \
         line."
      }
      Self::FindSymbol => {
        "Find where a type, function, method, constant or other item is defined, from the code index: path:line, \
         kind and signature, best matches first. Faster than searching file contents for a definition."
      }
      Self::SearchExternalFile => {
        "Search the files under a folder outside the worktree, such as a dependency's sources, for literal text. \
         Runs at once in the run's sources; elsewhere a person is asked first. Credential locations are refused."
      }
      Self::ReadExternalFile => {
        "Read a text file outside the worktree, such as a dependency's source, by absolute path. Runs at once in \
         the run's sources; elsewhere a person is asked first. Credential locations are refused."
      }
      Self::RunCheck => {
        "Run one of the checks the project offers this step (tests, lint, build) by name, with its fixed \
         arguments, in the worktree: its exit code and the end of its output. A person may be asked first."
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
    !matches!(
      self,
      Self::EditFile
        | Self::CreateFile
        | Self::DeleteFile
        | Self::MoveFile
        | Self::StopRunProcess
        | Self::CallRunApp
        | Self::StartQaApp
        | Self::RunCheck
        | Self::SaveMemory
        | Self::ReportProgress
    )
  }
}

/// The input schema of `tool`; `projects` are the names and descriptions of the checks `run_check` may run.
pub(super) fn input_schema(tool: OrchesterTool, projects: &[(String, String, bool)]) -> Value {
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
    OrchesterTool::DeleteFile => fixed(DELETE_FILE),
    OrchesterTool::MoveFile => fixed(MOVE_FILE),
    OrchesterTool::FindReferences => fixed(FIND_REFERENCES),
    OrchesterTool::GetTask | OrchesterTool::ListRunProcesses => fixed(NO_INPUT),
    OrchesterTool::StopRunProcess => json!({"type":"object","properties":{
      "pid":{"type":"integer","minimum":1,"maximum":u32::MAX,
        "description":"The process id from list_run_processes. Only this run's executable paths may be stopped."}
    },"required":["pid"],"additionalProperties":false}),
    OrchesterTool::CallRunApp => json!({"type":"object","properties":{
      "pid":{"type":["integer","null"],"minimum":1,"maximum":u32::MAX,
        "description":"The pid of the app this run started; null for the QA app start_qa_app started in this step."},
      "tool":{"type":"string","minLength":1,"maxLength":200,"description":"The embedded MCP tool name, e.g. lurq_windows, navigate or lurq_screenshot."},
      "arguments":{"type":"object","description":"That tool's arguments; use {} for none. Never a discovery token."}
    },"required":["tool","arguments"],"additionalProperties":false}),
    OrchesterTool::StartQaApp => fixed(START_QA_APP),
    OrchesterTool::SaveMemory => fixed(SAVE_MEMORY),
    OrchesterTool::ReportProgress => fixed(REPORT_PROGRESS),
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
    OrchesterTool::RunCheck => {
      let names: Vec<&str> = projects.iter().map(|(name, ..)| name.as_str()).collect();
      json!({"type":"object","properties":{
        "name":{"type":"string","enum":names,"description":project_list(projects)}
      },"required":["name"],"additionalProperties":false})
    }
  }
}

/// The checks, one per line with what each does, for the description of `name`.
fn project_list(projects: &[(String, String, bool)]) -> String {
  let mut text = "The check to run:".to_owned();
  for (name, description, _) in projects {
    text.push_str("\n- ");
    text.push_str(name);
    if !description.is_empty() {
      text.push_str(": ");
      text.push_str(description);
    }
  }
  text
}
