//! The MCP servers a profile may choose, shared by the profile editor and the domain tools.
use orchester_core::{connection::mcp::McpServer, resource::AgentProfile};

use crate::messages::Message;

/// Refused when `profile` chooses a server the project no longer has (`servers`), or adds one that is
/// turned off. A server `kept` already chose stays while it is off: turning a server off never blocks
/// saving the profiles that chose it, and a disabled server is given to no agent.
pub fn check(servers: &[McpServer], kept: Option<&AgentProfile>, profile: &AgentProfile) -> Result<(), Message> {
  for id in profile.mcp_servers().ids() {
    let Some(server) = servers.iter().find(|server| server.id() == *id) else {
      return Err(Message::new("appMcp.errors.mcpServerMissing").with("server", id.to_string()));
    };
    let added = !kept.is_some_and(|kept| kept.mcp_servers().ids().contains(id));
    if added && !server.enabled() {
      return Err(Message::new("appMcp.errors.mcpServerOff").with("server", server.name().to_string()));
    }
  }
  Ok(())
}
