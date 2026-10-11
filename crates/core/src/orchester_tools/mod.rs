//! Orchester's own typed tools for agents (`docs/interactive-tool-approvals.md`, "Orchester's tools"): the worktree's
//! files, files outside it, the step's skills, the Tools library's command tools offered to the step (checks), the
//! run's task, the task's and the project's memory, and the step's progress line. An agent CLI cannot tell what a
//! shell line does, so every such line outside its allowed commands asks a person; a typed tool says exactly what it
//! does, so Orchester decides here whether a call runs, asks or is not offered at all ([`decide`]).
//!
//! One set serves every agent: Claude Code and Codex get it as the tools of the MCP server [`SERVER`]
//! that the runner already serves for each invocation (`crate::connection::claude_approval::relay`). No tool takes a
//! project or a run: the relay belongs to one invocation, and every worktree path is relative to its worktree.
//!
//! - Each tool has a name, a description, a strict input schema ([`catalog`]) and an access class ([`AccessClass`]); a
//!   call's input is read strictly into its type ([`ToolCall::parse`]).
//! - The worktree's files are read, searched, edited line by line and created ([`FileCall`]): an edit replaces lines
//!   only when they are what the agent read ([`replace_lines`]).
//! - What a project tool runs is a program with its arguments as separate values ([`ToolWork`]), never a shell line, in
//!   the worktree with the environment the step's agent gets, within a time limit; its output is cut to a size
//!   ([`ToolResult`]) and its exit code returned.
//! - A read or search outside the worktree never reaches a credential location, a `.git` folder or Orchester's own
//!   files, and is cut to a size ([`OutsideReads`]).
//! - The step's skills are listed and read by name, each within its own files ([`StepSkills`]).

mod app;
mod catalog;
mod changes;
mod files;
mod index;
mod input;
mod lines;
mod output;
mod outside;
mod policy;
mod processes;
mod skills;
mod task_view;
mod work;

use serde_json::Value;
use thiserror::Error;

pub use self::{
  app::{AppCall, AppDiscovery, AppDiscoveryError},
  changes::changed_files,
  files::{DEFAULT_LIMIT_FILES, FileCall, LineEdit, MAX_LIMIT_FILES, NewFile, ReadLines, SearchTerm},
  index::{CodeIndex, FindSymbol, MAX_HITS, SymbolHit, plain_relative, shown_hits},
  input::{Revision, ToolCall},
  lines::{LineEditError, range_now, replace_lines},
  output::{MAX_ANSWER_BYTES, OutputKept, ProgramOutput, ToolResult},
  outside::{MAX_LINE_BYTES, MAX_READ_BYTES, OutsideReadError, OutsideReads},
  policy::{StepTools, ToolPolicy, auto_approvable, decide},
  processes::{ProcessCall, ProcessScopeError, RunProcessScope},
  skills::{SkillCall, SkillOfferError, SkillReadError, SkillSource, StepSkills},
  task_view::TaskView,
  work::{ExternalReach, ProjectTool, ToolContext, ToolProgram, ToolWork},
};
use crate::connection::stream::{FileChange, FileChangeKind, NativeCall};

/// The MCP server that serves the tools, the one that serves Claude Code's permission prompt.
pub const SERVER: &str = crate::connection::claude_approval::SERVER;

/// The tools Orchester offers agents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OrchesterTool {
  ReadFile,
  ListFiles,
  SearchFile,
  EditFile,
  CreateFile,
  DeleteFile,
  MoveFile,
  FindSymbol,
  FindReferences,
  SearchExternalFile,
  ReadExternalFile,
  RunCheck,
  ListRunProcesses,
  StopRunProcess,
  CallRunApp,
  ListSkills,
  ReadSkill,
  GetTask,
  SaveMemory,
  ReportProgress,
}

/// What a tool may touch, which decides whether its calls ask ([`decide`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessClass {
  /// Reads the worktree and its repository, changes nothing.
  ReadOnly,
  /// Changes the worktree.
  WritesWorktree,
  /// Reaches outside the worktree, or may: another folder, the network, a program the project chose.
  Outside,
  /// Keeps something with the run or the project (memory, the step's progress line); changes no file.
  Records,
  /// Lists, stops or drives app processes whose resolved executable stays in this run's worktree.
  RunProcesses,
}

impl OrchesterTool {
  pub const ALL: [Self; 20] = [
    Self::ReadFile,
    Self::ListFiles,
    Self::SearchFile,
    Self::EditFile,
    Self::CreateFile,
    Self::DeleteFile,
    Self::MoveFile,
    Self::FindSymbol,
    Self::FindReferences,
    Self::SearchExternalFile,
    Self::ReadExternalFile,
    Self::RunCheck,
    Self::ListRunProcesses,
    Self::StopRunProcess,
    Self::CallRunApp,
    Self::ListSkills,
    Self::ReadSkill,
    Self::GetTask,
    Self::SaveMemory,
    Self::ReportProgress,
  ];

  /// The name the agent calls the tool by.
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::ReadFile => "read_file",
      Self::ListFiles => "list_files",
      Self::SearchFile => "search_file",
      Self::EditFile => "edit_file",
      Self::CreateFile => "create_file",
      Self::DeleteFile => "delete_file",
      Self::MoveFile => "move_file",
      Self::FindSymbol => "find_symbol",
      Self::FindReferences => "find_references",
      Self::SearchExternalFile => "search_external_file",
      Self::ReadExternalFile => "read_external_file",
      Self::RunCheck => "run_check",
      Self::ListRunProcesses => "list_run_processes",
      Self::StopRunProcess => "stop_run_process",
      Self::CallRunApp => "call_run_app",
      Self::ListSkills => "list_skills",
      Self::ReadSkill => "read_skill",
      Self::GetTask => "get_task",
      Self::SaveMemory => "save_memory",
      Self::ReportProgress => "report_progress",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|tool| tool.as_str() == name)
  }

  /// The name an agent CLI calls the tool by: `mcp__Orchester__<tool>` for Claude Code, its own for Codex.
  pub fn called(self, claude: bool) -> String {
    if claude {
      format!("mcp__{SERVER}__{}", self.as_str())
    } else {
      self.as_str().to_owned()
    }
  }

  /// The tool Claude Code calls `name`: it names the tools of [`SERVER`] `mcp__Orchester__<tool>`.
  pub fn from_claude_name(name: &str) -> Option<Self> {
    Self::from_name(name.strip_prefix("mcp__")?.strip_prefix(SERVER)?.strip_prefix("__")?)
  }

  /// Whether the tool works on the worktree's files ([`FileCall`]).
  pub const fn is_file_tool(self) -> bool {
    matches!(
      self,
      Self::ReadFile
        | Self::ListFiles
        | Self::SearchFile
        | Self::EditFile
        | Self::CreateFile
        | Self::DeleteFile
        | Self::MoveFile
    )
  }

  /// Whether the tool reads outside the worktree, in a folder its call names: a person's approval of one call can
  /// cover later calls in that folder (*Approve similar*), and the run's sources let it run without asking.
  pub const fn reads_outside(self) -> bool {
    matches!(self, Self::SearchExternalFile | Self::ReadExternalFile)
  }

  /// The files a call with `input` changed once it succeeded, at the paths the agent gave: `edit_file` modifies the
  /// file at its `filename`, `create_file` adds one there, `delete_file` deletes it, `move_file` deletes `from` and
  /// adds `to`. First-edit tracking reads only the paths (`crate::run::FirstEditTracker`), and the transcript shows
  /// the call's own row.
  pub fn changed_files(self, input: &Value) -> Vec<FileChange> {
    let at = |field: &str, kind: FileChangeKind| {
      let path = input.get(field)?.as_str()?.to_owned();
      Some(FileChange { path, kind })
    };
    let changes = match self {
      Self::EditFile => vec![at("filename", FileChangeKind::Modified)],
      Self::CreateFile => vec![at("filename", FileChangeKind::Added)],
      Self::DeleteFile => vec![at("filename", FileChangeKind::Deleted)],
      Self::MoveFile => vec![at("from", FileChangeKind::Deleted), at("to", FileChangeKind::Added)],
      _ => Vec::new(),
    };
    changes.into_iter().flatten().collect()
  }

  /// How an agent CLI's call of this tool with `input` reads in the transcript: as the native agent's call of the
  /// same kind of file tool, with what its input asked. `None` for another tool, and for input the tool refuses: that
  /// call fails with the error its row shows.
  pub fn shown_call(self, input: &Value) -> Option<NativeCall> {
    use crate::native::tools::ToolName as NativeTool;
    // The refusal reaches the agent as the call's error result; the row shows that result.
    let call = FileCall::read(self, input)?.ok()?;
    let tool = match &call {
      FileCall::Read(_) => NativeTool::ReadFile,
      FileCall::List(_) => NativeTool::ListFiles,
      FileCall::Search(_) => NativeTool::Search,
      FileCall::Edit(_) => NativeTool::EditFile,
      FileCall::Create(_) => NativeTool::WriteFile,
      FileCall::Delete(_) | FileCall::Move { .. } => return None,
    };
    Some(NativeCall {
      tool,
      detail: None,
      declined: None,
      input: Some(call.call_input()?),
    })
  }

  pub const fn access(self) -> AccessClass {
    match self {
      Self::ReadFile | Self::ListFiles | Self::SearchFile | Self::FindSymbol | Self::FindReferences => {
        AccessClass::ReadOnly
      }
      Self::ListSkills | Self::ReadSkill | Self::GetTask => AccessClass::ReadOnly,
      Self::EditFile | Self::CreateFile | Self::DeleteFile | Self::MoveFile => AccessClass::WritesWorktree,
      Self::SearchExternalFile | Self::ReadExternalFile | Self::RunCheck => AccessClass::Outside,
      Self::SaveMemory | Self::ReportProgress => AccessClass::Records,
      Self::ListRunProcesses | Self::StopRunProcess | Self::CallRunApp => AccessClass::RunProcesses,
    }
  }
}

/// Why a call is not run. The message is the error result the agent gets.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ToolCallError {
  #[error("there is no Orchester tool named `{0}`")]
  UnknownTool(String),
  #[error("`{}` is not available in this step", .0.as_str())]
  NotOffered(OrchesterTool),
  #[error("invalid input for `{tool}`: {message}", tool = .tool.as_str())]
  Invalid { tool: OrchesterTool, message: String },
  #[error("the project offers no check named `{0}` to this step")]
  UnknownCheck(String),
  #[error("the program `{0}` of the check was not found")]
  ProgramNotFound(String),
  #[error(
    "the program `{0}` of the check now leads into the project's repository or off this computer, so it was not      started; a person can choose it again on the project's Tools page"
  )]
  ProgramMoved(String),
  #[error(transparent)]
  Outside(#[from] OutsideReadError),
  #[error(transparent)]
  Skill(#[from] SkillReadError),
}
