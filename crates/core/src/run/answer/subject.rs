//! What a request asks to do, as the answers judge it: a command, classified, or a call of one tool of
//! an MCP server.

use std::path::Path;

use crate::{
  permission::category::{ClassifiedCommand, CommandScope},
  run::PendingToolApproval,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApprovalSubject {
  Command(ClassifiedCommand),
  McpTool { server: String, tool: String },
}

impl ApprovalSubject {
  /// `approval`'s subject; a command is classified as it runs in `scope`, in the working folder the
  /// request names (Codex reports it; for Claude Code it is the worktree), else in the worktree.
  pub fn of(approval: &PendingToolApproval, scope: &CommandScope) -> Self {
    if approval.command {
      let folder = approval.target.as_deref().map_or(scope.worktree(), Path::new);
      Self::Command(ClassifiedCommand::classify_in(&approval.action, folder, scope))
    } else {
      Self::McpTool {
        server: approval.server.clone(),
        tool: approval.action.clone(),
      }
    }
  }
}
