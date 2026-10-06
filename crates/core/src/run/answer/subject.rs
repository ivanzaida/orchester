//! What a request asks to do, as the answers judge it: a command, classified, or a call of one tool of
//! an MCP server.

use std::path::Path;

use crate::{
  native::command::CommandSyntax,
  permission::category::{ClassifiedCommand, CommandScope},
  run::{CommandShell, NativeCommandAsk, PendingToolApproval},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApprovalSubject {
  Command(ClassifiedCommand),
  McpTool { server: String, tool: String },
}

impl ApprovalSubject {
  /// `approval`'s subject; a command is classified as it runs in `scope`, in the working folder the
  /// request names (Codex reports it; for Claude Code it is the worktree), else in the worktree. A
  /// native agent's command is read with the grammar of what runs it ([`native_command`]).
  pub fn of(approval: &PendingToolApproval, scope: &CommandScope) -> Self {
    if approval.command {
      let folder = approval.target.as_deref().map_or(scope.worktree(), Path::new);
      Self::Command(match approval.native {
        Some(native) => native_command(&approval.action, native, folder, scope),
        None => ClassifiedCommand::classify_in(&approval.action, folder, scope),
      })
    } else {
      Self::McpTool {
        server: approval.server.clone(),
        tool: approval.action.clone(),
      }
    }
  }
}

/// A native agent's command `text`: PowerShell's on Windows, zsh's (read as Bash) on macOS, or without a
/// shell the arguments as this platform splits them, which is how the agent's command was started.
fn native_command(text: &str, native: NativeCommandAsk, folder: &Path, scope: &CommandScope) -> ClassifiedCommand {
  match native.shell {
    Some(CommandShell::Pwsh | CommandShell::WindowsPowerShell) => {
      ClassifiedCommand::classify_powershell_in(text, folder, scope)
    }
    Some(CommandShell::Zsh) => ClassifiedCommand::classify_in(text, folder, scope),
    None => match CommandSyntax::HOST.split(text) {
      Ok(argv) => ClassifiedCommand::classify_argv_in(text, &argv, folder, scope),
      Err(_) => ClassifiedCommand::unread(text),
    },
  }
}
