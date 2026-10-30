//! The servers Codex gets by another name than the project's (`orchester_core::connection::invocation::CodexMcp::
//! named_apart`, when Codex's own configuration has a server of that name): what Codex says of one (the server of an
//! MCP tool call or of an elicitation, one it could not start) names the server the profile chose, so the step's
//! activity, its approvals and their rules, and the tools that run without asking go by the project's name.

use orchester_core::connection::app_server_mcp::McpInventoryError;
use serde_json::{Value, json};

use super::AppServerSession;

/// Where a notification or request of Codex names an MCP server: an `mcpToolCall` item, an elicitation.
const SERVER_FIELDS: [&str; 2] = ["/params/item/server", "/params/serverName"];

impl AppServerSession {
  /// `message`, a notification or request of Codex, as a line with each server Codex got by another name named as
  /// the project names it; `None` when it names no such server.
  pub(super) fn by_project_names(&self, message: &Value) -> Option<String> {
    if self.setup.renamed.is_empty() {
      return None;
    }
    let mut message = message.clone();
    let mut renamed = false;
    for pointer in SERVER_FIELDS {
      let Some(field) = message.pointer_mut(pointer) else {
        continue;
      };
      if let Some(project) = field.as_str().and_then(|name| self.setup.renamed.get(name)) {
        *field = json!(project);
        renamed = true;
      }
    }
    renamed.then(|| message.to_string())
  }

  /// The project's name of the server Codex gets by `name`.
  pub(super) fn project_name(&self, name: String) -> String {
    self.setup.renamed.get(&name).cloned().unwrap_or(name)
  }

  /// `error` about a selected server, naming it as the project does.
  pub(super) fn project_named(&self, error: McpInventoryError) -> McpInventoryError {
    match error {
      McpInventoryError::MissingSelected(name) => McpInventoryError::MissingSelected(self.project_name(name)),
      McpInventoryError::UnavailableSelected(mut server) => {
        server.name = self.project_name(server.name);
        McpInventoryError::UnavailableSelected(server)
      }
      other => other,
    }
  }
}
