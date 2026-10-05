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

mod catalog;
mod git;
mod input;
mod output;
mod outside;
mod policy;
mod work;

use thiserror::Error;

pub use self::{
  git::GIT_TIMEOUT,
  input::{CommitMessage, GitCall, RepoPath, Revision, ToolCall},
  output::{OutputKept, ToolResult},
  outside::{OutsideReadError, OutsideReads},
  policy::{StepTools, ToolPolicy, decide},
  work::{ProjectTool, ToolContext, ToolProgram, ToolWork},
};

/// The MCP server that serves the tools, the one that serves Claude Code's permission prompt.
pub const SERVER: &str = crate::connection::claude_approval::SERVER;

/// The tools Orchester offers agents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OrchesterTool {
  GitStatus,
  GitDiff,
  GitLog,
  GitShow,
  GitAdd,
  GitCommit,
  ReadOutsideFile,
  RunProjectTool,
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
  pub const ALL: [Self; 8] = [
    Self::GitStatus,
    Self::GitDiff,
    Self::GitLog,
    Self::GitShow,
    Self::GitAdd,
    Self::GitCommit,
    Self::ReadOutsideFile,
    Self::RunProjectTool,
  ];

  /// The name the agent calls the tool by.
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::GitStatus => "git_status",
      Self::GitDiff => "git_diff",
      Self::GitLog => "git_log",
      Self::GitShow => "git_show",
      Self::GitAdd => "git_add",
      Self::GitCommit => "git_commit",
      Self::ReadOutsideFile => "read_outside_file",
      Self::RunProjectTool => "run_project_tool",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|tool| tool.as_str() == name)
  }

  pub const fn access(self) -> AccessClass {
    match self {
      Self::GitStatus | Self::GitDiff | Self::GitLog | Self::GitShow => AccessClass::ReadOnly,
      Self::GitAdd | Self::GitCommit => AccessClass::WritesWorktree,
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
  #[error(transparent)]
  Outside(#[from] OutsideReadError),
}
