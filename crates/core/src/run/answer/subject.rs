//! What a request asks to do, as the answers judge it: a command, classified, or a call of one tool of
//! an MCP server.

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
  /// `approval`'s subject; a command is classified as it runs in `scope`.
  pub fn of(approval: &PendingToolApproval, scope: &CommandScope) -> Self {
    if approval.command {
      Self::Command(ClassifiedCommand::classify(&approval.action, scope))
    } else {
      Self::McpTool {
        server: approval.server.clone(),
        tool: approval.action.clone(),
      }
    }
  }
}
