//! Orchester's own typed tools for agents (`docs/interactive-tool-approvals.md`, "Orchester's tools"): git in
//! the worktree, a read of a file outside it, and the Tools library's command tools. An agent CLI cannot tell
//! what a shell line does, so every such line outside its allowed commands asks a person; a typed tool says
//! exactly what it does, so Orchester decides here whether a call runs, asks or is not offered at all
//! ([`decide`]).
//!
//! One set serves every agent: Claude Code and Codex get it as the tools of the MCP server [`SERVER`]
//! that the runner already serves for each invocation (`crate::connection::claude_approval::relay`),
//! and a native agent can declare the same schemas in its own request and run the same calls.
//!
//! - Each tool has a name, a description, a strict input schema ([`catalog`]) and an access class ([`AccessClass`]); a
//!   call's input is read strictly into its type ([`ToolCall::parse`]).
//! - What a call runs is a program with its arguments as separate values ([`ToolWork`]), never a shell line: an
//!   argument starting with `-` or holding `;`, `$(…)` or quotes stays one value of the program. It runs in the
//!   worktree with the environment the step's agent gets, which holds no secret value, within a time limit; its output
//!   is cut to a size ([`ToolResult`]) and its exit code returned.
//! - A read outside the worktree never reaches a credential location, a `.git` folder or Orchester's own files, and is
//!   cut to a size ([`OutsideReads`]).
//! - The step's skills are listed and read by name, each within its own files ([`StepSkills`]).

mod catalog;
mod git;
mod index;
mod input;
mod link;
mod output;
mod outside;
mod policy;
mod skills;
mod work;

use serde_json::Value;
use thiserror::Error;

pub use self::{
  git::{GIT_TIMEOUT, changed_files},
  index::{CodeIndex, FindSymbol, MAX_HITS, SymbolHit, plain_relative, shown_hits},
  input::{CommitMessage, GitCall, RepoPath, Revision, ToolCall},
  output::{MAX_ANSWER_BYTES, OutputKept, ProgramOutput, ToolResult},
  outside::{MAX_LINE_BYTES, MAX_READ_BYTES, OutsideReadError, OutsideReads},
  policy::{StepTools, ToolPolicy, auto_approvable, decide},
  skills::{SkillCall, SkillOfferError, SkillReadError, SkillSource, StepSkills},
  work::{ProjectTool, ToolContext, ToolProgram, ToolWork},
};
use crate::{
  connection::stream::{CallInput, FileChange, FileChangeKind, NativeCall},
  native::tools::ToolName as NativeTool,
};

/// The MCP server that serves the tools, the one that serves Claude Code's permission prompt.
pub const SERVER: &str = crate::connection::claude_approval::SERVER;

/// The tools Orchester offers agents. The file tools are the native agent's (`crate::native::tools`), with
/// its schemas and inputs; the runner runs them with the native agent's executors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OrchesterTool {
  ReadFile,
  ListFiles,
  Search,
  EditFile,
  WriteFile,
  FindSymbol,
  GitStatus,
  GitDiff,
  GitLog,
  GitShow,
  GitAdd,
  GitCommit,
  ReadOutsideFile,
  RunProjectTool,
  ListSkills,
  ReadSkill,
}

/// What a tool may touch, which decides whether its calls ask ([`decide`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessClass {
  /// Reads the worktree and its repository, changes nothing.
  ReadOnly,
  /// Changes the worktree or its branch.
  WritesWorktree,
  /// Reaches outside the worktree, or may: another folder, the network, a program the project chose.
  Outside,
}

impl OrchesterTool {
  pub const ALL: [Self; 16] = [
    Self::ReadFile,
    Self::ListFiles,
    Self::Search,
    Self::EditFile,
    Self::WriteFile,
    Self::FindSymbol,
    Self::GitStatus,
    Self::GitDiff,
    Self::GitLog,
    Self::GitShow,
    Self::GitAdd,
    Self::GitCommit,
    Self::ReadOutsideFile,
    Self::RunProjectTool,
    Self::ListSkills,
    Self::ReadSkill,
  ];

  /// The name the agent calls the tool by.
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::ReadFile => NativeTool::ReadFile.as_str(),
      Self::ListFiles => NativeTool::ListFiles.as_str(),
      Self::Search => NativeTool::Search.as_str(),
      Self::EditFile => NativeTool::EditFile.as_str(),
      Self::WriteFile => NativeTool::WriteFile.as_str(),
      Self::FindSymbol => "find_symbol",
      Self::GitStatus => "git_status",
      Self::GitDiff => "git_diff",
      Self::GitLog => "git_log",
      Self::GitShow => "git_show",
      Self::GitAdd => "git_add",
      Self::GitCommit => "git_commit",
      Self::ReadOutsideFile => "read_outside_file",
      Self::RunProjectTool => "run_project_tool",
      Self::ListSkills => "list_skills",
      Self::ReadSkill => "read_skill",
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

  /// The file a call with `input` changed once it succeeded, at the path the agent gave: `edit_file` and
  /// `write_file` change the file at their `path`. Their answer does not say whether `write_file` made the
  /// file, so the change reads as modified; first-edit tracking reads only the path
  /// (`crate::run::FirstEditTracker`), and the transcript shows the call's own row.
  pub fn changed_file(self, input: &Value) -> Option<FileChange> {
    match self {
      Self::EditFile | Self::WriteFile => Some(FileChange {
        path: input.get("path")?.as_str()?.to_owned(),
        kind: FileChangeKind::Modified,
      }),
      _ => None,
    }
  }

  /// How an agent CLI's call of this tool with `input` reads in the transcript: as the native agent's call of the
  /// same file tool, with what its input asked. `None` for another tool, and for input the tool refuses: that call
  /// fails with the error its row shows.
  pub fn shown_call(self, input: &Value) -> Option<NativeCall> {
    let tool = self.file_tool()?;
    // The refusal reaches the agent as the call's error result; the row shows that result.
    let input = crate::native::tools::parse_call(tool.as_str(), input).ok()?;
    Some(NativeCall {
      tool,
      detail: None,
      declined: None,
      input: Some(CallInput::of(&input)?),
    })
  }

  /// The native agent's tool this is, for a file tool.
  pub const fn file_tool(self) -> Option<NativeTool> {
    match self {
      Self::ReadFile => Some(NativeTool::ReadFile),
      Self::ListFiles => Some(NativeTool::ListFiles),
      Self::Search => Some(NativeTool::Search),
      Self::EditFile => Some(NativeTool::EditFile),
      Self::WriteFile => Some(NativeTool::WriteFile),
      _ => None,
    }
  }

  pub const fn access(self) -> AccessClass {
    match self {
      Self::ReadFile | Self::ListFiles | Self::Search | Self::FindSymbol => AccessClass::ReadOnly,
      Self::GitStatus | Self::GitDiff | Self::GitLog | Self::GitShow => AccessClass::ReadOnly,
      Self::ListSkills | Self::ReadSkill => AccessClass::ReadOnly,
      Self::EditFile | Self::WriteFile | Self::GitAdd | Self::GitCommit => AccessClass::WritesWorktree,
      Self::ReadOutsideFile | Self::RunProjectTool => AccessClass::Outside,
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
  #[error("the project offers no tool named `{0}` to this step")]
  UnknownProjectTool(String),
  #[error("the program `{0}` of the project tool was not found")]
  ProgramNotFound(String),
  #[error("`{0}` is or lies in a repository of its own, which Orchester's tools do not stage")]
  NestedRepository(String),
  #[error("`{0}` is a folder: name the files to stage, as git_status lists them")]
  Folder(String),
  #[error("`{0}` is or passes through a link, which Orchester's tools do not stage")]
  Link(String),
  #[error(transparent)]
  Outside(#[from] OutsideReadError),
  #[error(transparent)]
  Skill(#[from] SkillReadError),
}
