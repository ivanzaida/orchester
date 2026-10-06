//! The prompt's *Files and commands* part: how the agent should read, change files and run commands so
//! that what the step's permissions allow runs without a person. Orchester's file tools
//! (`crate::orchester_tools`, served to both CLIs) never ask where they are offered, while a shell edit, a
//! `cd` prefix or a chain of commands does not match the allowed commands one by one and waits for a person.
//! Each CLI is told the tools by the names it sees. Each sentence names only what the step's CLI and permissions have:
//! a read-only step is not told to edit.

use crate::{
  connection::AgentProvider,
  permission::{AgentPermissions, CommandPermission, RepositoryAccess},
};

/// What the agent's CLI and the step's effective permissions give it to work with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkingTools {
  cli: Cli,
  edits: bool,
  commands: Commands,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cli {
  ClaudeCode,
  Codex,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Commands {
  /// The allowed commands run, others ask.
  Listed,
  /// Every command asks.
  AskEach,
  /// Commands run in the worktree's sandbox.
  Sandboxed,
}

impl WorkingTools {
  /// What an agent of `provider` gets with `permissions`; `None` for a native agent, whose own prompt
  /// names its tools.
  pub fn new(provider: AgentProvider, permissions: &AgentPermissions) -> Option<Self> {
    let cli = match provider {
      AgentProvider::ClaudeCodeCli => Cli::ClaudeCode,
      AgentProvider::CodexCli => Cli::Codex,
      AgentProvider::AnthropicApi | AgentProvider::OpenAiApi => return None,
    };
    let commands = match permissions.commands() {
      CommandPermission::Allowed(list) if !list.is_empty() => Commands::Listed,
      CommandPermission::Allowed(_) | CommandPermission::AskBefore => Commands::AskEach,
      CommandPermission::AnyInWorktree => Commands::Sandboxed,
    };
    Some(Self {
      cli,
      edits: permissions.repository() == RepositoryAccess::Write,
      commands,
    })
  }

  /// The part's text.
  pub(super) fn text(self) -> String {
    let files = match (self.cli, self.edits) {
      (Cli::ClaudeCode, true) => {
        "Read, search and change files with Orchester's file tools: mcp__Orchester__read_file, \
         mcp__Orchester__list_files, mcp__Orchester__search, mcp__Orchester__edit_file (after reading the file) and \
         mcp__Orchester__write_file. Never through the shell: no `cat`, `sed`, `awk` or heredocs."
      }
      (Cli::ClaudeCode, false) => {
        "Read and search files with Orchester's file tools: mcp__Orchester__read_file, mcp__Orchester__list_files \
         and mcp__Orchester__search, not through the shell (`cat`, `sed`, `awk`)."
      }
      (Cli::Codex, true) => {
        "Read, search and change files with the read_file, list_files, search, edit_file (after read_file) and \
         write_file tools of the Orchester MCP server, not through the shell (`cat`, `sed -i`, heredocs, `>`)."
      }
      (Cli::Codex, false) => {
        "Read and search files with the read_file, list_files and search tools of the Orchester MCP server, not \
         through the shell (`cat`, `sed`, `awk`)."
      }
    };
    let commands = match self.commands {
      Commands::Listed => {
        "Commands already run in the task's worktree: never start one with `cd`. Run one command per call, bare, \
         without `;`, `&&` or variables, so that it matches the allowed commands; other commands wait for a \
         person's approval."
      }
      Commands::AskEach => {
        "Commands already run in the task's worktree: never start one with `cd`. Each command waits for a \
         person's approval: run only those the step needs."
      }
      Commands::Sandboxed => "Commands already run in the task's worktree: never start one with `cd`.",
    };
    [files, commands]
      .into_iter()
      .filter(|line| !line.is_empty())
      .collect::<Vec<_>>()
      .join("\n")
  }
}
