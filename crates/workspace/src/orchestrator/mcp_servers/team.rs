//! The chat's MCP server tools in a team project. Its MCP servers are the team's, versioned on the team server, and the
//! app's own tools change them there: `add_mcp_server` creates the team's server as its version 1, `update_mcp_server`
//! publishes its next version, `remove_mcp_server` archives it and `set_mcp_server_enabled` turns it on or off on this
//! computer. A change is not proposed as a card: the card shows a server of this computer, and the team's is read from
//! the team server. What a change widens is those tools' to judge, as for any client of the app: they refuse it as
//! `person_only` unless a person lets automation change agent permissions, and a member is refused `not_admin`. The
//! chat calls them at once while it drives and in a turn the person's message started; in a turn nobody asked for
//! while it does not drive, the change waits for the person (`waits_for_person`), and the agent tells them what to ask.

use orchester_store::ProjectDb;
use rmcp::model::JsonObject;

use super::{
  super::{
    acts::drives,
    tools::{ChatScope, app_tool},
  },
  Acted,
};
use crate::{
  app_mcp::{Services, ToolError, Translate, blocking},
  messages::Message,
};

/// Whether `scope`'s project is linked to a team server.
pub(super) async fn of(scope: &ChatScope) -> Result<bool, ToolError> {
  let root = scope.root.clone();
  blocking(move || Ok(ProjectDb::open(&root)?.sharing()?.is_some())).await
}

/// Calls the app's MCP server tool `tool` with `arguments` for the team project of `scope`, or refuses the call in a
/// turn nobody asked for while the chat does not drive.
pub(super) async fn act(
  (services, translate): (&Services, &Translate),
  scope: &ChatScope,
  tool: &str,
  arguments: JsonObject,
) -> Result<Acted, ToolError> {
  if scope.on_its_own && !drives(services, scope).await? {
    return Err(ToolError::refused(
      "waits_for_person",
      vec![Message::new("orchestrator.tools.teamMcpWaitsForPerson")],
    ));
  }
  Ok(Acted::Made(
    app_tool((services, translate), tool, arguments).await,
    None,
  ))
}
