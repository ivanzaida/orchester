//! The prompt's *Files and commands* part: how the agent should read, change files and run commands so
//! that what the step's permissions allow runs without a person. Orchester's file tools
//! (`crate::orchester_tools`, served to both CLIs) never ask where they are offered, while a shell edit, a
//! `cd` prefix or a chain of commands does not match the allowed commands one by one and waits for a person.
//! Each CLI is told the tools by the names it sees. Each sentence names only what the step's CLI and permissions have:
//! a read-only step is not told to edit, nor how to merge its base branch when that moves (ORC-54).
//!
//! Every step of either CLI is also told how to run commands ([`RUN_PLAINLY`], ORC-95), so a project works with auto
//! mode without an instruction of its own: a plain command is one Orchester's command reader reads to the end
//! ([`crate::permission::category`]), which the allowed commands and auto mode's hard rules can judge. A project
//! instruction that says the same again is accepted: the part does not read the project's instructions.

use crate::{
  connection::AgentProvider,
  permission::{AgentPermissions, CommandPermission, RepositoryAccess},
};

/// How every agent CLI is told to run commands, whatever the step's permissions.
pub const RUN_PLAINLY: &str = "Run each command plainly, so Orchester can read it to the end: one command per call, \
                               with no `;`, `&&`, `||`, pipes, variables, `$(...)`, redirections or loops. Read and \
                               search files, dependency sources outside the worktree too, with the file tools instead \
                               of the shell. Never use `git stash`: its stack is shared by every worktree of the \
                               repository.";

/// Where a writing step keeps its scratch files.
pub const SCRATCH_FILES: &str = "Put scratch files under .tmp/ in the worktree, not in the system's temporary folder.";

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

  fn processes(self) -> &'static str {
    match self.cli {
      Cli::ClaudeCode => {
        "Find apps you started with mcp__Orchester__list_run_processes and stop them with mcp__Orchester__stop_run_process(pid), instead of Stop-Process or kill. These tools run without asking and refuse executables outside this run's worktree, the runner and the agent CLI. Stop only apps you started. Drive that app with mcp__Orchester__call_run_app(pid, tool, arguments) instead of running tools/lurq-mcp-client.py. The relay finds discovery itself, keeps its token private and returns PNG paths under .tmp/."
      }
      Cli::Codex => {
        "Find apps you started with list_run_processes and stop them with stop_run_process(pid) of the Orchester MCP server, instead of Stop-Process or kill. These tools run without asking and refuse executables outside this run's worktree, the runner and the agent CLI. Stop only apps you started. Drive that app with call_run_app(pid, tool, arguments) instead of running tools/lurq-mcp-client.py. The relay finds discovery itself, keeps its token private and returns PNG paths under .tmp/."
      }
    }
  }

  /// The part's text.
  pub(super) fn text(self) -> String {
    let files = match (self.cli, self.edits) {
      (Cli::ClaudeCode, true) => {
        "Read, search and change files only with Orchester's file tools: mcp__Orchester__read_file, \
         mcp__Orchester__list_files, mcp__Orchester__search_file, mcp__Orchester__edit_file (after reading the \
         file: give the lines to replace and, as expected, exactly what they hold now), mcp__Orchester__create_file \
         (new files only), mcp__Orchester__delete_file and mcp__Orchester__move_file (one file each, never a \
         folder). Claude Code's own Read, Edit, Write, NotebookEdit, Grep and Glob are not available. Never read or \
         change files through the shell: no `cat`, `sed`, `awk`, heredocs, `rm` or `mv`. \
         mcp__Orchester__read_file shows a PNG, JPEG or WebP file as the image: open screenshots with it."
      }
      (Cli::ClaudeCode, false) => {
        "Read and search files only with Orchester's file tools: mcp__Orchester__read_file, \
         mcp__Orchester__list_files and mcp__Orchester__search_file. Claude Code's own Read, Grep and Glob are not \
         available; do not read files through the shell (`cat`, `sed`, `awk`). mcp__Orchester__read_file shows a \
         PNG, JPEG or WebP file as the image: open screenshots with it."
      }
      (Cli::Codex, true) => {
        "Read, search and change files with the read_file, list_files, search_file, edit_file (after read_file: give \
         the lines to replace and, as expected, exactly what they hold now), create_file (new files only), delete_file \
         and move_file (one file each, never a folder) tools of the Orchester MCP server, not through the shell \
         (`cat`, `rg`, `sed -i`, heredocs, `>`, `rm`, `mv`)."
      }
      (Cli::Codex, false) => {
        "Read and search files with the read_file, list_files and search_file tools of the Orchester MCP server, not \
         through the shell (`cat`, `rg`, `sed`, `awk`)."
      }
    };
    let commands = match self.commands {
      Commands::Listed => {
        "Commands already run in the task's worktree: never start one with `cd`. A plain command matches the allowed \
         commands; other commands wait for a person's approval."
      }
      Commands::AskEach => {
        "Commands already run in the task's worktree: never start one with `cd`. Each command waits for a \
         person's approval: run only those the step needs."
      }
      Commands::Sandboxed => "Commands already run in the task's worktree: never start one with `cd`.",
    };
    let (scratch, merge) = if self.edits {
      (SCRATCH_FILES, super::MERGE_SAFELY)
    } else {
      ("", "")
    };
    [files, commands, RUN_PLAINLY, scratch, merge, self.processes()]
      .into_iter()
      .filter(|line| !line.is_empty())
      .collect::<Vec<_>>()
      .join("\n")
  }
}
